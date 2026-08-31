const INDEXER_URL =
  process.env.NEXT_PUBLIC_INDEXER_URL || "http://localhost:3001";

export interface PoolData {
  id: string;
  token: string;
  balance: bigint;
  adminCount: number;
  threshold: number;
}

/**
 * Fetch the set of post IDs that a user has liked.
 * Returns a Set for O(1) lookup performance.
 * Throws an error if the indexer is unreachable.
 */
export async function fetchUserLikes(userAddress: string): Promise<Set<string>> {
  const res = await fetch(`${INDEXER_URL}/api/likes/${userAddress}`);
  if (!res.ok) {
    throw new Error(`Failed to fetch likes: ${res.status} ${res.statusText}`);
  }
  const data = await res.json();
  const likes: string[] = data.likes ?? data.post_ids ?? [];
  return new Set(likes.map(String));
}

/**
 * Fetch all pools from the indexer.
 * Throws an error if the indexer is unreachable or returns an error response.
 */
export async function fetchPools(): Promise<PoolData[]> {
  const res = await fetch(`${INDEXER_URL}/api/pools`);
  if (!res.ok) {
    throw new Error(`Indexer returned ${res.status}: ${res.statusText}`);
  }
  const data = await res.json();
  const list = Array.isArray(data) ? data : data.pools ?? [];
  return list.map((p: any) => ({
    id: p.pool_id ?? p.id,
    token: p.token,
    balance: BigInt(p.balance ?? 0),
    adminCount: Array.isArray(p.admins) ? p.admins.length : (p.admin_count ?? 0),
    threshold: p.threshold ?? 1,
  }));
}
