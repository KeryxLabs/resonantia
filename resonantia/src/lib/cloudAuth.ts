import { Clerk } from '@clerk/clerk-js';
import { getClerkPublishableKey, getClerkGatewayTokenTemplate } from '$lib/config';

export type CloudAuthStatus = {
  available: boolean;
  signedIn: boolean;
  userId: string | null;
  username: string | null;
  reason?: string;
};

let clerkPromise: Promise<Clerk | null> | null = null;

function normalizeGatewayBaseUrl(value: string): string {
  const trimmed = value.trim().replace(/\/+$/, '');
  if (!trimmed) {
    return '';
  }

  if (/^https?:\/\//i.test(trimmed)) {
    return trimmed;
  }

  if (trimmed.startsWith('//')) {
    if (typeof window !== 'undefined') {
      return `${window.location.protocol}${trimmed}`;
    }
    return `https:${trimmed}`;
  }

  if (trimmed.startsWith('/')) {
    return trimmed;
  }

  // Avoid route-relative fetches like `gateway/api/...` from `/account`.
  return `/${trimmed}`;
}

async function loadClerk(): Promise<Clerk | null> {
  if (typeof window === 'undefined') {
    return null;
  }

  const publishableKey = getClerkPublishableKey();
  if (!publishableKey) {
    return null;
  }

  if (!clerkPromise) {
    clerkPromise = (async () => {
      const clerk = new Clerk(publishableKey);
      await clerk.load();
      return clerk;
    })().catch((err) => {
      clerkPromise = null; // reset so next call retries
      console.error('[resonantia] Clerk failed to load:', err);
      return null;
    });
  }

  return clerkPromise;
}

export async function getCloudAuthStatus(): Promise<CloudAuthStatus> {
  const clerk = await loadClerk();
  if (!clerk) {
    return {
      available: false,
      signedIn: false,
      userId: null,
      username: null,
      reason: getClerkPublishableKey() ? 'clerk_unavailable' : 'missing_publishable_key',
    };
  }

  const signedIn = Boolean(clerk.session && clerk.user);
  return {
    available: true,
    signedIn,
    userId: clerk.user?.id ?? null,
    username: clerk.user?.username ?? null,
  };
}

export async function startCloudSignIn(): Promise<void> {
  const clerk = await loadClerk();
  if (!clerk) {
    throw new Error('Clerk is not configured. Set VITE_CLERK_PUBLISHABLE_KEY first.');
  }

  await clerk.openSignIn({});
}

/**
 * Redirect-based sign-in — safe on standalone pages (no UI component mount needed).
 * Clerk hosted sign-in completes then redirects back to `returnUrl`.
 */
export async function redirectToCloudSignIn(returnUrl?: string): Promise<void> {
  const clerk = await loadClerk();
  if (!clerk) {
    throw new Error('Clerk is not configured. Set VITE_CLERK_PUBLISHABLE_KEY first.');
  }

  await clerk.redirectToSignIn({
    redirectUrl: returnUrl ?? window.location.href,
  });
}

export async function getGatewayAuthToken(): Promise<string> {
  const tokenTemplate = getClerkGatewayTokenTemplate();
  const clerk = await loadClerk();
  if (!clerk) {
    throw new Error('Clerk is not configured. Check window.__resonantia__.clerkPublishableKey.');
  }

  if (!clerk.session) {
    throw new Error('No active Clerk session. Sign in first.');
  }

  // Force a fresh token so API calls never reuse an expired cached JWT.
  const token = tokenTemplate
    ? await clerk.session.getToken({ template: tokenTemplate, skipCache: true })
    : await clerk.session.getToken({ skipCache: true });

  if (!token) {
    throw new Error('Clerk did not return a gateway auth token for the active session.');
  }

  return token;
}

export async function signOutCloud(): Promise<void> {
  const clerk = await loadClerk();
  if (!clerk) {
    return;
  }

  await clerk.signOut();
}

export type CloudAccount = {
  userId: string;
  tier: string;
  memberSince: string;
};

export async function getCloudAccount(
  gatewayBaseUrl: string,
  gatewayAuthToken: string
): Promise<CloudAccount | null> {
  if (!gatewayBaseUrl || !gatewayAuthToken) {
    return null;
  }

  const base = normalizeGatewayBaseUrl(gatewayBaseUrl);
  if (!base) {
    return null;
  }
  try {
    const res = await fetch(`${base}/api/v1/account`, {
      headers: { Authorization: `Bearer ${gatewayAuthToken}` },
    });
    if (!res.ok) return null;
    return (await res.json()) as CloudAccount;
  } catch {
    return null;
  }
}

export async function createCustomerPortal(
  gatewayBaseUrl: string,
  gatewayAuthToken: string
): Promise<string> {
  if (!gatewayBaseUrl || !gatewayAuthToken) {
    throw new Error('Gateway URL and auth token are required for the billing portal.');
  }

  const base = normalizeGatewayBaseUrl(gatewayBaseUrl);
  if (!base) {
    throw new Error('Gateway URL is not configured.');
  }
  const res = await fetch(`${base}/api/v1/customer-portal`, {
    method: 'POST',
    headers: { Authorization: `Bearer ${gatewayAuthToken}` },
  });

  if (!res.ok) {
    const body = await res.text().catch(() => '');
    throw new Error(`Billing portal failed: ${res.status} ${body}`.trim());
  }

  const data = (await res.json()) as { url: string };
  if (!data.url) throw new Error('No billing portal URL returned from gateway.');
  return data.url;
}

export async function createCheckoutSession(
  gatewayBaseUrl: string,
  gatewayAuthToken: string,
  tier: 'resonant' | 'soulful'
): Promise<string> {
  if (!gatewayBaseUrl || !gatewayAuthToken) {
    throw new Error('Gateway URL and auth token are required for checkout.');
  }

  const base = normalizeGatewayBaseUrl(gatewayBaseUrl);
  if (!base) {
    throw new Error('Gateway URL is not configured.');
  }
  const res = await fetch(`${base}/api/v1/checkout`, {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${gatewayAuthToken}`,
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({ tier }),
  });

  if (!res.ok) {
    const body = await res.text().catch(() => '');
    throw new Error(`Checkout session failed: ${res.status} ${body}`.trim());
  }

  const data = (await res.json()) as { url: string };
  if (!data.url) {
    throw new Error('No checkout URL returned from gateway.');
  }

  return data.url;
}
