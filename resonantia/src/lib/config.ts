declare global {
  interface Window {
    __resonantia__?: {
      gatewayBaseUrl?: string;
      clerkPublishableKey?: string;
      clerkGatewayTokenTemplate?: string;
    };
  }
}

const UNRESOLVED_TEMPLATE_RE = /^\$\{[A-Z0-9_]+\}$/;

function cleanValue(value?: string): string {
  const normalized = (value ?? '').trim();
  if (!normalized || UNRESOLVED_TEMPLATE_RE.test(normalized)) {
    return '';
  }
  return normalized;
}

function runtimeConfig() {
  if (typeof window !== 'undefined' && window.__resonantia__) {
    return window.__resonantia__;
  }
  return {};
}

export function getGatewayBaseUrl(): string {
  return cleanValue(runtimeConfig().gatewayBaseUrl) || cleanValue(import.meta.env.VITE_GATEWAY_BASE_URL);
}

export function getClerkPublishableKey(): string {
  return (
    cleanValue(runtimeConfig().clerkPublishableKey) || cleanValue(import.meta.env.VITE_CLERK_PUBLISHABLE_KEY)
  );
}

export function getClerkGatewayTokenTemplate(): string {
  return (
    cleanValue(runtimeConfig().clerkGatewayTokenTemplate) ||
    cleanValue(import.meta.env.VITE_CLERK_GATEWAY_TOKEN_TEMPLATE)
  );
}
