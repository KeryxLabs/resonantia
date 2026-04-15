import { Clerk } from '@clerk/clerk-js';

export type CloudAuthStatus = {
  available: boolean;
  signedIn: boolean;
  userId: string | null;
  reason?: string;
};

const publishableKey = (import.meta.env.VITE_CLERK_PUBLISHABLE_KEY ?? '').trim();
const tokenTemplate = (import.meta.env.VITE_CLERK_GATEWAY_TOKEN_TEMPLATE ?? '').trim();

let clerkPromise: Promise<Clerk | null> | null = null;

async function loadClerk(): Promise<Clerk | null> {
  if (typeof window === 'undefined') {
    return null;
  }

  if (!publishableKey) {
    return null;
  }

  if (!clerkPromise) {
    clerkPromise = (async () => {
      const clerk = new Clerk(publishableKey);
      await clerk.load();
      return clerk;
    })();
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
      reason: publishableKey ? 'clerk_unavailable' : 'missing_publishable_key',
    };
  }

  const signedIn = Boolean(clerk.session && clerk.user);
  return {
    available: true,
    signedIn,
    userId: clerk.user?.id ?? null,
  };
}

export async function startCloudSignIn(): Promise<void> {
  const clerk = await loadClerk();
  if (!clerk) {
    throw new Error('Clerk is not configured. Set VITE_CLERK_PUBLISHABLE_KEY first.');
  }

  await clerk.openSignIn({});
}

export async function getGatewayAuthToken(): Promise<string> {
  const clerk = await loadClerk();
  if (!clerk) {
    throw new Error('Clerk is not configured. Set VITE_CLERK_PUBLISHABLE_KEY first.');
  }

  if (!clerk.session) {
    throw new Error('No active Clerk session. Sign in first.');
  }

  const token = tokenTemplate
    ? await clerk.session.getToken({ template: tokenTemplate })
    : await clerk.session.getToken();

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

  const base = gatewayBaseUrl.replace(/\/$/, '');
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
