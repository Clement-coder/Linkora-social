"use client";

import { useEffect, useState } from "react";
import { PostCard, Post } from "./PostCard";
import { fetchIsPaused } from "../lib/api";

/** How often to re-check the contract's pause status while the feed is mounted. */
const PAUSE_POLL_INTERVAL_MS = 30_000;

interface FeedProps {
  posts: Post[];
  loading?: boolean;
  onLike?: (postId: number) => void;
  onTip?: (postId: number) => void;
  likedPosts?: Set<number>;
}

export function Feed({ posts, loading, onLike, onTip, likedPosts = new Set() }: FeedProps) {
  // Fetch on bootstrap and keep polling so the banner reflects pause/unpause
  // without requiring a page reload.
  const [paused, setPaused] = useState(false);

  useEffect(() => {
    let cancelled = false;

    const check = async () => {
      const isPaused = await fetchIsPaused();
      if (!cancelled) setPaused(isPaused);
    };

    check();
    const interval = setInterval(check, PAUSE_POLL_INTERVAL_MS);
    return () => {
      cancelled = true;
      clearInterval(interval);
    };
  }, []);

  // Re-check immediately before submitting a write, to catch the contract
  // being paused between polls (race condition), and only proceed if clear.
  const guardedWrite = async (action: () => void) => {
    const isPaused = await fetchIsPaused();
    setPaused(isPaused);
    if (isPaused) return;
    action();
  };

  // Create guarded callback wrappers for PostCard
  const createGuardedCallback = (callback: ((postId: number) => void) | undefined, postId: number) => {
    if (!callback) return undefined;
    return () => guardedWrite(() => callback(postId));
  };

  if (loading) {
    return (
      <div style={styles.container}>
        {[1, 2, 3].map((i) => (
          <div key={i} style={styles.skeleton}></div>
        ))}
      </div>
    );
  }

  if (posts.length === 0) {
    return (
      <div style={styles.empty}>
        <div style={styles.emptyIcon}>📝</div>
        <h3>No posts yet</h3>
        <p style={styles.emptyText}>Be the first to share something!</p>
      </div>
    );
  }

  return (
    <div style={styles.container}>
      {paused && (
        <div style={styles.pausedBanner} role="alert">
          Linkora is temporarily paused. Writes are disabled until the protocol resumes.
        </div>
      )}
      {posts.map((post) => (
        <div key={post.id} style={styles.postWrap}>
          <PostCard 
            post={post}
            onLike={createGuardedCallback(onLike, Number(post.id))}
            onTip={createGuardedCallback(onTip, Number(post.id))}
            isLiked={likedPosts.has(Number(post.id))}
            disabled={paused}
          />
        </div>
      ))}
    </div>
  );
}

const styles: Record<string, React.CSSProperties> = {
  container: {
    maxWidth: "600px",
    width: "100%",
    margin: "0 auto",
    padding: "var(--spacing-md)",
  },
  skeleton: {
    height: "200px",
    background: "var(--color-bg-secondary)",
    borderRadius: "12px",
    marginBottom: "var(--spacing-md)",
    animation: "pulse 1.5s ease-in-out infinite",
  },
  postWrap: {
    marginBottom: "var(--spacing-md)",
  },
  pausedBanner: {
    background: "var(--color-warning-bg, #fff3cd)",
    color: "var(--color-warning-text, #664d03)",
    border: "1px solid var(--color-warning-border, #ffe69c)",
    borderRadius: "8px",
    padding: "var(--spacing-sm) var(--spacing-md)",
    marginBottom: "var(--spacing-md)",
    fontSize: "0.9rem",
  },
  empty: {
    textAlign: "center",
    padding: "var(--spacing-xl)",
    color: "var(--color-text-secondary)",
  },
  emptyIcon: {
    fontSize: "3rem",
    marginBottom: "var(--spacing-md)",
  },
  emptyText: {
    marginTop: "var(--spacing-sm)",
  },
};
