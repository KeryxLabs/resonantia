use std::env;
use std::time::Instant;

use axum::extract::{Request, State};
use axum::http::{HeaderMap, HeaderValue};
use axum::middleware::Next;
use axum::response::Response;
use opentelemetry::global;
use opentelemetry::trace::TracerProvider as _;
use opentelemetry::KeyValue;
use opentelemetry_http::HeaderExtractor;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::propagation::TraceContextPropagator;
use opentelemetry_sdk::runtime::Tokio;
use opentelemetry_sdk::{trace as sdktrace, Resource};
use sha2::{Digest, Sha256};
use tracing::{error, info, warn, Instrument};
use tracing_opentelemetry::OpenTelemetrySpanExt;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;
use uuid::Uuid;

use crate::GatewayContext;

#[derive(Clone)]
pub(crate) struct ObservabilityConfig {
    pub(crate) request_log_sample_rate: f64,
}

pub(crate) struct TelemetryRuntime {
    pub(crate) otel_enabled: bool,
}

fn parse_sample_rate_env(name: &str, default: f64) -> f64 {
    env::var(name)
        .ok()
        .and_then(|value| value.trim().parse::<f64>().ok())
        .map(|value| value.clamp(0.0, 1.0))
        .unwrap_or(default)
}

pub(crate) fn read_observability_config() -> ObservabilityConfig {
    let request_log_sample_rate = parse_sample_rate_env(
        "RESONANTIA_GATEWAY_OBS_REQUEST_LOG_SAMPLE_RATE",
        0.2,
    );

    info!(request_log_sample_rate, "gateway request observability configured");
    ObservabilityConfig {
        request_log_sample_rate,
    }
}

fn extract_request_id(headers: &HeaderMap) -> Option<String> {
    headers
        .get("x-request-id")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string())
}

fn generate_request_id() -> String {
    Uuid::new_v4().to_string()
}

fn should_sample(seed: &str, sample_rate: f64) -> bool {
    if sample_rate <= 0.0 {
        return false;
    }
    if sample_rate >= 1.0 {
        return true;
    }

    let mut hasher = Sha256::new();
    hasher.update(seed.as_bytes());
    let digest = hasher.finalize();

    let mut slice = [0u8; 8];
    slice.copy_from_slice(&digest[..8]);
    let unit = (u64::from_be_bytes(slice) as f64) / (u64::MAX as f64);
    unit < sample_rate
}

pub(crate) async fn observability_middleware(
    State(context): State<GatewayContext>,
    request: Request,
    next: Next,
) -> Response {
    let started = Instant::now();
    let method = request.method().to_string();
    let path = request.uri().path().to_string();
    let client = crate::client_kind(request.headers());
    let request_id = extract_request_id(request.headers()).unwrap_or_else(generate_request_id);

    let traceparent = request
        .headers()
        .get("traceparent")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string());

    let span = tracing::info_span!(
        "http.request",
        request_id = %request_id,
        method = %method,
        path = %path,
        client = %client,
        traceparent = tracing::field::Empty,
    );

    if let Some(traceparent_value) = traceparent.as_deref() {
        span.record("traceparent", tracing::field::display(traceparent_value));
    }

    global::get_text_map_propagator(|propagator| {
        let parent = propagator.extract(&HeaderExtractor(request.headers()));
        span.set_parent(parent);
    });

    let mut response = next.run(request).instrument(span).await;
    let status_code = response.status().as_u16();
    let duration_ms = started.elapsed().as_millis() as u64;

    if let Ok(value) = HeaderValue::from_str(&request_id) {
        response.headers_mut().insert("x-request-id", value);
    }

    let should_log = status_code >= 500
        || should_sample(&request_id, context.observability.request_log_sample_rate);

    if should_log {
        if status_code >= 500 {
            error!(
                request_id = %request_id,
                method = %method,
                path = %path,
                client = %client,
                status_code,
                duration_ms,
                "http request completed with server error"
            );
        } else if status_code >= 400 {
            warn!(
                request_id = %request_id,
                method = %method,
                path = %path,
                client = %client,
                status_code,
                duration_ms,
                "http request completed with client error"
            );
        } else {
            info!(
                request_id = %request_id,
                method = %method,
                path = %path,
                client = %client,
                status_code,
                duration_ms,
                "http request completed"
            );
        }
    }

    response
}

pub(crate) fn init_tracing() -> TelemetryRuntime {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("resonantia_gateway=info,tower_http=info"));

    let otel_service_name = env::var("RESONANTIA_OTEL_SERVICE_NAME")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "resonantia-gateway".to_string());

    let otlp_endpoint = env::var("RESONANTIA_OTEL_EXPORTER_OTLP_ENDPOINT")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    let otel_trace_sample_rate = parse_sample_rate_env("RESONANTIA_OTEL_TRACE_SAMPLE_RATE", 0.1);

    global::set_text_map_propagator(TraceContextPropagator::new());

    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_target(false)
        .compact();

    if let Some(endpoint) = otlp_endpoint {
        let resource = Resource::new(vec![
            KeyValue::new("service.name", otel_service_name.clone()),
            KeyValue::new("service.namespace", "resonantia"),
        ]);

        let exporter = opentelemetry_otlp::SpanExporter::builder()
            .with_tonic()
            .with_endpoint(endpoint.clone())
            .build()
            .expect("failed to build OTLP span exporter");

        let tracer_provider = sdktrace::TracerProvider::builder()
            .with_sampler(sdktrace::Sampler::TraceIdRatioBased(otel_trace_sample_rate))
            .with_resource(resource)
            .with_batch_exporter(exporter, Tokio)
            .build();

        let tracer = tracer_provider.tracer(otel_service_name.clone());
        global::set_tracer_provider(tracer_provider);

        tracing_subscriber::registry()
            .with(filter)
            .with(fmt_layer)
            .with(tracing_opentelemetry::layer().with_tracer(tracer))
            .init();

        info!(
            %endpoint,
            service = %otel_service_name,
            otel_trace_sample_rate,
            "opentelemetry exporter enabled"
        );

        return TelemetryRuntime { otel_enabled: true };
    }

    tracing_subscriber::registry()
        .with(filter)
        .with(fmt_layer)
        .init();

    info!("opentelemetry exporter disabled (set RESONANTIA_OTEL_EXPORTER_OTLP_ENDPOINT to enable)");
    TelemetryRuntime { otel_enabled: false }
}

#[cfg(test)]
mod tests {
    use axum::http::{HeaderMap, HeaderValue};

    use super::{extract_request_id, should_sample};

    #[test]
    fn should_sample_handles_boundary_rates() {
        assert!(!should_sample("seed", 0.0));
        assert!(should_sample("seed", 1.0));
    }

    #[test]
    fn extract_request_id_trims_and_ignores_empty_values() {
        let mut headers = HeaderMap::new();
        headers.insert("x-request-id", HeaderValue::from_static("  req-123  "));
        assert_eq!(extract_request_id(&headers), Some("req-123".to_string()));

        headers.insert("x-request-id", HeaderValue::from_static("   "));
        assert_eq!(extract_request_id(&headers), None);
    }
}
