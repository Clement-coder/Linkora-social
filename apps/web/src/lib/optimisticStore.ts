"use client";

import { useSyncExternalStore } from "react";

/* ────────────────────────────────────────────────────────────────────────── */
/*  Types                                                                    */
/* ────────────────────────────────────────────────────────────────────────── */

export type FollowState = {
  isFollowing: boolean;
  followersCount: number;
  followingCount: number;
};

export type LikeState = {
  isLiked: boolean;
  likeCount: number;
};

export type TipState = {
  tipTotal: number;
};

/* ────────────────────────────────────────────────────────────────────────── */
/*  Store internals                                                          */
/* ────────────────────────────────────────────────────────────────────────── */

// Key format: `${followerAddress}:${followeeAddress}`
const followStateMap = new Map<string, FollowState>();
// Key format: `${userAddress}:${postId}`
const likeStateMap = new Map<string, LikeState>();
// Key format: postId string
const tipStateMap = new Map<string, TipState>();
const listeners = new Set<() => void>();

function subscribe(listener: () => void) {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

function notify() {
  for (const listener of listeners) {
    listener();
  }
}

/* ────────────────────────────────────────────────────────────────────────── */
/*  Public API                                                               */
/* ────────────────────────────────────────────────────────────────────────── */

const pendingMap = new Map<string, boolean>();
const legacyFollowingMap = new Map<string, boolean>();

export const OptimisticStore = {
  setFollowState(key: string, state: FollowState) {
    followStateMap.set(key, state);
    notify();
  },

  getFollowState(key: string): FollowState | undefined {
    return followStateMap.get(key);
  },

  clearFollowState(key: string) {
    followStateMap.delete(key);
    notify();
  },

  setLikeState(key: string, state: LikeState) {
    likeStateMap.set(key, state);
    notify();
  },

  getLikeState(key: string): LikeState | undefined {
    return likeStateMap.get(key);
  },

  setTipState(key: string, state: TipState) {
    tipStateMap.set(key, state);
    notify();
  },

  getTipState(key: string): TipState | undefined {
    return tipStateMap.get(key);
  },

  clearLikeState(key: string) {
    likeStateMap.delete(key);
    notify();
  },

  clearTipState(key: string) {
    tipStateMap.delete(key);
    notify();
  },

  /**
   * Reconcile optimistic state against a fresh server response.
   *
   * Resolution rules:
   * 1. For each likeStateMap entry whose postId IS in visiblePosts,
   *    delete it — the component will fall back to `initialState` which
   *    reflects the server-confirmed value (server wins).
   * 2. For each likeStateMap entry whose postId is NOT in visiblePosts,
   *    delete it — the post has been filtered out and the optimistic
   *    entry should not persist.
   * 3. Entries belonging to a different userAddress are left untouched.
   * 4. Same rules apply for tipStateMap (keyed by postId string).
   */
  reconcileFeed(userAddress: string | null, visiblePosts: Array<{ id: string | number }>) {
    const visibleIds = new Set(visiblePosts.map((p) => String(p.id)));
    let changed = false;

    // Reconcile like state — keys are `${userAddress}:${postId}`
    if (userAddress) {
      for (const key of [...likeStateMap.keys()]) {
        const separatorIdx = key.indexOf(":");
        if (separatorIdx === -1) continue;

        const keyUser = key.slice(0, separatorIdx);
        if (keyUser !== userAddress) continue;

        // Post is either present (server wins) or absent (filtered out) — either way, drop it
        likeStateMap.delete(key);
        changed = true;
      }
    }

    // Reconcile tip state — keys are postId strings
    for (const key of [...tipStateMap.keys()]) {
      // Only prune entries for posts we have a definitive answer about.
      // If the post is in the visible set, server truth is now available
      // via initialState. If absent, the optimistic entry is stale.
      tipStateMap.delete(key);
      changed = true;
    }

    if (changed) {
      notify();
    }
  },

  // Legacy API for FollowList.tsx
  subscribe,
  isFollowing(targetAddress: string): boolean {
    return legacyFollowingMap.get(targetAddress) ?? false;
  },
  setFollowing(targetAddress: string, isFollowing: boolean) {
    legacyFollowingMap.set(targetAddress, isFollowing);
    notify();
  },
  isPending(targetAddress: string): boolean {
    return pendingMap.get(targetAddress) ?? false;
  },
  setPending(targetAddress: string, pending: boolean | { isPending: boolean }) {
    pendingMap.set(targetAddress, typeof pending === "boolean" ? pending : pending.isPending);
    notify();
  },
};

/* ────────────────────────────────────────────────────────────────────────── */
/*  Hooks                                                                    */
/* ────────────────────────────────────────────────────────────────────────── */

/**
 * Returns the optimistic follow state if one exists, otherwise falls back
 * to `initialState` (the "truth" from the server/contract).
 */
export function useOptimisticFollow(
  follower: string | null,
  followee: string,
  initialState: FollowState
): FollowState {
  const key = `${follower}:${followee}`;

  const optimistic = useSyncExternalStore(
    subscribe,
    () => (follower ? OptimisticStore.getFollowState(key) : undefined),
    () => undefined
  );

  return optimistic ?? initialState;
}

/**
 * Returns the optimistic like state if one exists, otherwise falls back
 * to `initialState`.
 */
export function useOptimisticLike(
  user: string | null,
  postId: string | bigint,
  initialState: LikeState
): LikeState {
  const key = `${user}:${postId}`;

  const optimistic = useSyncExternalStore(
    subscribe,
    () => (user ? OptimisticStore.getLikeState(key) : undefined),
    () => undefined
  );

  return optimistic ?? initialState;
}

/**
 * Returns the optimistic tip state if one exists, otherwise falls back
 * to `initialState`.
 */
export function useOptimisticTip(postId: string | bigint, initialState: TipState): TipState {
  const key = String(postId);

  const optimistic = useSyncExternalStore(
    subscribe,
    () => OptimisticStore.getTipState(key),
    () => undefined
  );

  return optimistic ?? initialState;
}
