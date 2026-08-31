"use client";

import React, { useState, useEffect, useRef, useCallback } from "react";
import Link from "next/link";
import { useWallet } from "@/hooks/useWallet";
import { PostCard, PostCardSkeleton, type Post } from "@/components/PostCard";
import { OptimisticStore, useOptimisticLike, useOptimisticTip } from "@/lib/optimisticStore";
import { LinkoraClient } from "../../../../../packages/sdk/src/client";
import { validateAmount, validateStellarAddress } from "@/lib/validate";
import { FieldError } from "@/components/forms/FieldError";
import { OnboardingGuard } from "@/components/onboarding/OnboardingGuard";
import { AnimatedList } from "@/components/AnimatedList";
import { fetchUserLikes } from "@/lib/api";

/* ────────────────────────────────────────────────────────────────────────── */
/*  Config & Constants                                                       */
/* ────────────────────────────────────────────────────────────────────────── */

const contractId = process.env.NEXT_PUBLIC_CONTRACT_ID || "CDUMMY";
const rpcUrl = process.env.NEXT_PUBLIC_RPC_URL || "https://soroban-testnet.stellar.org";
const indexerUrl = process.env.NEXT_PUBLIC_INDEXER_URL || "http://localhost:3001";
const PAGE_SIZE = 10;

/* ────────────────────────────────────────────────────────────────────────── */
/*  Interactive Post Wrapper to bind Optimistic Updates                     */
/* ────────────────────────────────────────────────────────────────────────── */

interface InteractivePostCardProps {
  post: Post;
  currentUserAddress: string | null;
  onTipClick: (post: Post) => void;
  userLikes: Set<string>;
}

function InteractivePostCard({ post, currentUserAddress, onTipClick, userLikes }: InteractivePostCardProps) {
  const [isTipping, setIsTipping] = useState(false);
  const postId = String(post.id);

  // Determine initial liked state from the fetched user likes
  const initialIsLiked = userLikes.has(postId);

  // Optimistic Like State
  const likeState = useOptimisticLike(currentUserAddress, postId, {
    isLiked: initialIsLiked,
    likeCount: Number(post.like_count ?? 0),
  });

  // Optimistic Tip State
  const tipState = useOptimisticTip(postId, {
    tipTotal: Number(post.tip_total ?? 0),
  });

  const handleLike = async () => {
    if (!currentUserAddress) {
      alert("Please connect your wallet to like posts.");
      return;
    }

    const key = `${currentUserAddress}:${post.id}`;
    const nextIsLiked = !likeState.isLiked;
    const nextLikeCount = likeState.likeCount + (nextIsLiked ? 1 : -1);

    // Apply optimistic update
    OptimisticStore.setLikeState(key, {
      isLiked: nextIsLiked,
      likeCount: nextLikeCount,
    });

    try {
      const client = new LinkoraClient({ contractId, rpcUrl });
      // In production, this XDR would be signed via Freighter and submitted.
      const _txXdr = client.likePost(currentUserAddress, Number(post.id));
      // Success - the optimistic update remains
    } catch (err) {
      console.error("Failed to like post on chain:", err);
      // Rollback optimistic update
      OptimisticStore.setLikeState(key, {
        isLiked: !nextIsLiked,
        likeCount: likeState.likeCount,
      });
    }
  };

  const enrichedPost = {
    ...post,
    like_count: likeState.likeCount,
    tip_total: tipState.tipTotal,
  };

  return (
    <PostCard
      post={enrichedPost}
      isLiked={likeState.isLiked}
      onLike={handleLike}
      onTip={() => onTipClick(post)}
      isTipping={isTipping}
    />
  );
}

/* ────────────────────────────────────────────────────────────────────────── */
/*  Main Feed Page Component                                                 */
/* ────────────────────────────────────────────────────────────────────────── */

export default function FeedPage() {
  const { address: currentUserAddress, connected, connect } = useWallet();
  const [activeTab, setActiveTab] = useState<"following" | "explore">("explore");

  // Feed items, pagination & loading states
  const [posts, setPosts] = useState<Post[]>([]);
  const [loading, setLoading] = useState(true);
  const [loadingMore, setLoadingMore] = useState(false);
  const [hasMore, setHasMore] = useState(true);
  const [cursor, setCursor] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [showScrollTop, setShowScrollTop] = useState(false);

  // User's liked posts set (for seeding initial like state)
  const [userLikes, setUserLikes] = useState<Set<string>>(new Set());
  const [likesLoading, setLikesLoading] = useState(false);

  // Real-time updates via WebSocket
  const [hasNewPosts, setHasNewPosts] = useState(false);

  // Whether the current user follows nobody (following tab empty state)
  const [followsNobody, setFollowsNobody] = useState(false);
  const socketRef = useRef<WebSocket | null>(null);

  // Tipping modal state
  const [tippingPost, setTippingPost] = useState<Post | null>(null);
  const [tipToken, setTipToken] = useState("");
  const [tipAmount, setTipAmount] = useState("");
  const [tipErrors, setTipErrors] = useState<{ token?: string; amount?: string }>({});
  const [tipSubmitting, setTipSubmitting] = useState(false);

  /* ── Fetch Posts Logic ──────────────────────────────────────────────── */

  // Fetch user's liked posts when connected
  useEffect(() => {
    if (!currentUserAddress) {
      setUserLikes(new Set());
      return;
    }

    setLikesLoading(true);
    fetchUserLikes(currentUserAddress)
      .then((likes) => {
        setUserLikes(likes);
        // Seed optimistic store with confirmed liked state
        likes.forEach((postId) => {
          const key = `${currentUserAddress}:${postId}`;
          const existing = OptimisticStore.getLikeState(key);
          if (!existing) {
            // Only seed if no optimistic update exists
            OptimisticStore.setLikeState(key, {
              isLiked: true,
              likeCount: 0, // Will be overwritten by post data
            });
          }
        });
      })
      .catch((err) => {
        console.error("Failed to fetch user likes:", err);
        // Fail gracefully - user will just not see liked state initially
      })
      .finally(() => {
        setLikesLoading(false);
      });
  }, [currentUserAddress]);

  /* ── Fetch Posts Logic ──────────────────────────────────────────────── */

  const fetchExploreFeed = useCallback(async (cursorParam: number | null, append = false) => {
    try {
      if (!append) setLoading(true);
      else setLoadingMore(true);

      const cursorQuery = cursorParam !== null ? `&cursor=${cursorParam}` : "";
      const res = await fetch(`${indexerUrl}/api/posts?limit=${PAGE_SIZE}${cursorQuery}`);
      if (!res.ok) throw new Error("Failed to fetch explore posts");

      const data = await res.json();
      const fetchedPosts: Post[] = data.posts ?? [];

      setPosts((prev) => (append ? [...prev, ...fetchedPosts] : fetchedPosts));
      setHasMore(data.has_more ?? false);
    } catch (err) {
      setError(err instanceof Error ? err.message : "An error occurred");
    } finally {
      setLoading(false);
      setLoadingMore(false);
    }
  }, []);

  const fetchFollowingFeed = useCallback(
    async (cursorParam: number | null, append = false) => {
      if (!currentUserAddress) {
        setPosts([]);
        setLoading(false);
        return;
      }

      try {
        if (!append) setLoading(true);
        else setLoadingMore(true);

        // 1. Get followed accounts
        const followingRes = await fetch(
          `${indexerUrl}/api/follows/${currentUserAddress}/following?limit=100`
        );
        if (!followingRes.ok) throw new Error("Failed to fetch following graph");
        const followingData = await followingRes.json();
        const followingList: string[] = followingData.following ?? [];

        if (followingList.length === 0) {
          setPosts([]);
          setHasMore(false);
          setFollowsNobody(true);
          setLoading(false);
          setLoadingMore(false);
          return;
        }
        setFollowsNobody(false);

        // 2. Fetch posts from followed accounts in parallel
        const postsPromises = followingList.map(async (addr) => {
          const cursorQuery = cursorParam !== null ? `&cursor=${cursorParam}` : "";
          const postsRes = await fetch(
            `${indexerUrl}/api/posts?author=${addr}&limit=10${cursorQuery}`
          );
          if (!postsRes.ok) return [];
          const d = await postsRes.json();
          return d.posts ?? [];
        });

        const allFetchedNested = await Promise.all(postsPromises);
        const allFetchedPosts: Post[] = allFetchedNested.flat();

        // Sort chronological descending
        allFetchedPosts.sort((a, b) => {
          const timeA = Number(a.created_at ?? a.timestamp ?? 0);
          const timeB = Number(b.created_at ?? b.timestamp ?? 0);
          return timeB - timeA;
        });

        // For following tab, we use client-side pagination with cursor
        const startIdx = append ? posts.length : 0;
        const paginated = allFetchedPosts.slice(startIdx, startIdx + PAGE_SIZE);
        setPosts((prev) => (append ? [...prev, ...paginated] : paginated));
        setHasMore(startIdx + paginated.length < allFetchedPosts.length);
      } catch (err) {
        setError(err instanceof Error ? err.message : "An error occurred");
      } finally {
        setLoading(false);
        setLoadingMore(false);
      }
    },
    [currentUserAddress, posts] as const
  );

  const loadFeed = useCallback(
    (cursorParam: number | null, append = false) => {
      setError(null);
      if (activeTab === "following") {
        fetchFollowingFeed(cursorParam, append);
      } else {
        fetchExploreFeed(cursorParam, append);
      }
    },
    [activeTab, fetchExploreFeed, fetchFollowingFeed]
  );

  // Initial load and tab switching
  useEffect(() => {
    setCursor(null);
    setHasNewPosts(false);
    setFollowsNobody(false);
    loadFeed(null, false);
  }, [activeTab, loadFeed]);

  // Infinite Scroll Sentinel
  const sentinelRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (loading || loadingMore || !hasMore) return;

    const observer = new IntersectionObserver(
      (entries) => {
        if (entries[0]?.isIntersecting) {
          // Use the oldest post's timestamp as the next cursor
          const oldestPost = posts[posts.length - 1];
          const nextCursor = oldestPost
            ? Number(oldestPost.created_at ?? oldestPost.timestamp ?? 0)
            : null;
          setCursor(nextCursor);
          loadFeed(nextCursor, true);
        }
      },
      { rootMargin: "150px" }
    );

    const el = sentinelRef.current;
    if (el) observer.observe(el);
    return () => {
      if (el) observer.unobserve(el);
    };
  }, [loading, loadingMore, hasMore, posts, loadFeed]);

  // Scroll to Top FAB
  useEffect(() => {
    const handleScroll = () => {
      setShowScrollTop(window.scrollY > 500);
    };

    window.addEventListener("scroll", handleScroll);
    return () => window.removeEventListener("scroll", handleScroll);
  }, []);

  const scrollToTop = () => {
    window.scrollTo({ top: 0, behavior: "smooth" });
  };

  /* ── WebSocket Setup for Real-time indicator ───────────────────────── */

  useEffect(() => {
    const wsUrl = indexerUrl.replace(/^http/, "ws") + "/ws";

    const connectWs = () => {
      const socket = new WebSocket(wsUrl);
      socketRef.current = socket;

      socket.onopen = () => {
        // Subscribe to PostCreated events
        socket.send(JSON.stringify({ action: "subscribe", types: ["PostCreated"] }));
      };

      socket.onmessage = (event) => {
        try {
          const msg = JSON.parse(event.data);
          if (msg.type === "PostCreated") {
            setHasNewPosts(true);
          }
        } catch {}
      };

      socket.onerror = () => {
        socket.close();
      };

      socket.onclose = () => {
        // Try reconnecting after 5 seconds
        setTimeout(connectWs, 5000);
      };
    };

    connectWs();

    return () => {
      if (socketRef.current) {
        socketRef.current.close();
      }
    };
  }, []);

  const handleRefreshFeed = () => {
    setCursor(null);
    setHasNewPosts(false);
    loadFeed(null, false);
  };

  /* ── Tipping Form Submission ────────────────────────────────────────── */

  const handleOpenTipModal = (post: Post) => {
    setTippingPost(post);
    setTipToken("");
    setTipAmount("");
    setTipErrors({});
  };

  const handleCloseTipModal = () => {
    setTippingPost(null);
  };

  const handleTipSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!tippingPost || !currentUserAddress) return;

    // Validate inputs
    const errors: { token?: string; amount?: string } = {};
    const addrResult = validateStellarAddress(tipToken);
    if (!addrResult.valid) errors.token = addrResult.error;

    const amountResult = validateAmount(tipAmount);
    if (!amountResult.valid) errors.amount = amountResult.error;

    if (Object.keys(errors).length > 0) {
      setTipErrors(errors);
      return;
    }

    setTipSubmitting(true);
    const amountVal = parseFloat(tipAmount);
    const postTipKey = String(tippingPost.id);

    // Optimistically update tip count
    const currentTipTotal = Number(tippingPost.tip_total ?? 0);
    const nextTipTotal = currentTipTotal + amountVal;
    OptimisticStore.setTipState(postTipKey, { tipTotal: nextTipTotal });

    try {
      const client = new LinkoraClient({ contractId, rpcUrl });
      // Build transaction XDR
      const _txXdr = client.tip(
        currentUserAddress,
        Number(tippingPost.id),
        tipToken.trim(),
        BigInt(Math.floor(amountVal * 10_000_000)) // convert to 7 decimals
      );

      handleCloseTipModal();
    } catch (err) {
      console.error("Tipping failed:", err);
      // Rollback
      OptimisticStore.setTipState(postTipKey, { tipTotal: currentTipTotal });
      alert("Tipping transaction failed to build.");
    } finally {
      setTipSubmitting(false);
    }
  };

  /* ── UI Render ──────────────────────────────────────────────────────── */

  return (
    <OnboardingGuard>
      <div className="min-h-screen bg-[var(--bg-primary)] text-[var(--text-primary)] pb-12">
        {/* Real-time Toast Banner */}
        {hasNewPosts && (
          <div className="fixed top-20 left-1/2 -translate-x-1/2 z-50 animate-bounce">
            <button
              onClick={handleRefreshFeed}
              className="flex items-center gap-2 bg-violet-600 hover:bg-violet-500 text-white font-semibold text-sm px-5 py-2.5 rounded-full shadow-xl transition-all border border-violet-400/30"
            >
              ✨ New posts available! Click to refresh
            </button>
          </div>
        )}

        <div className="mx-auto max-w-2xl px-4 py-8">
          <header className="mb-8">
            <h1 className="text-3xl font-extrabold tracking-tight mb-6">Home Feed</h1>

            {/* Navigation Tabs */}
            <div className="flex border-b border-[var(--border)] gap-6">
              <button
                onClick={() => setActiveTab("explore")}
                className={`pb-3 text-base font-semibold transition-all relative ${
                  activeTab === "explore"
                    ? "text-[var(--foreground)]"
                    : "text-[var(--text-muted)] hover:text-[var(--foreground)]"
                }`}
              >
                Explore
                {activeTab === "explore" && (
                  <span className="absolute bottom-0 left-0 right-0 h-0.5 bg-violet-500 rounded-full" />
                )}
              </button>
              <button
                onClick={() => setActiveTab("following")}
                className={`pb-3 text-base font-semibold transition-all relative ${
                  activeTab === "following"
                    ? "text-[var(--foreground)]"
                    : "text-[var(--text-muted)] hover:text-[var(--foreground)]"
                }`}
              >
                Following
                {activeTab === "following" && (
                  <span className="absolute bottom-0 left-0 right-0 h-0.5 bg-violet-500 rounded-full" />
                )}
              </button>
            </div>
          </header>

          {/* Auth required wall for Following Tab */}
          {activeTab === "following" && (!connected || !currentUserAddress) ? (
            <div className="rounded-2xl border border-[var(--border)] bg-[var(--muted)] p-6 md:p-8 text-center shadow-lg">
              <div className="text-4xl mb-4">🔒</div>
              <h2 className="text-xl font-bold mb-2">Connect Your Wallet</h2>
              <p className="text-[var(--text-muted)] mb-6 max-w-sm mx-auto">
                Follow developers and creators to view a personalized feed of their latest posts.
              </p>
              <button
                onClick={connect}
                className="bg-violet-600 hover:bg-violet-500 text-white font-semibold px-6 py-2.5 rounded-xl transition-all shadow-md"
              >
                Connect Wallet
              </button>
            </div>
          ) : (
            <>
              {/* Error Message */}
              {error && (
                <div
                  className="bg-red-950/40 border border-red-800 text-red-200 px-4 py-3 rounded-xl mb-4 text-sm"
                  role="alert"
                >
                  ⚠️ {error}
                </div>
              )}

              {/* Skeletons on initial load */}
              {loading ? (
                <div className="space-y-4">
                  <PostCardSkeleton />
                  <PostCardSkeleton />
                  <PostCardSkeleton />
                </div>
              ) : posts.length === 0 ? (
                /* Empty state */
                <div className="rounded-2xl border border-[var(--border)] bg-[var(--muted)]/50 p-12 text-center">
                  {activeTab === "following" && followsNobody ? (
                    <>
                      <div className="mx-auto mb-4 flex h-16 w-16 items-center justify-center rounded-full bg-[var(--bg-tertiary)]">
                        <svg
                          className="w-8 h-8 text-[var(--text-muted)]"
                          fill="none"
                          viewBox="0 0 24 24"
                          strokeWidth={1.5}
                          stroke="currentColor"
                          aria-hidden="true"
                        >
                          <path
                            strokeLinecap="round"
                            strokeLinejoin="round"
                            d="M15 19.128a9.38 9.38 0 002.625.372 9.337 9.337 0 004.121-.952 4.125 4.125 0 00-7.533-2.493M15 19.128v-.003c0-1.113-.285-2.16-.786-3.07M15 19.128v.106A12.318 12.318 0 018.624 21c-2.331 0-4.512-.645-6.374-1.766l-.001-.109a6.375 6.375 0 0111.964-3.07M12 6.375a3.375 3.375 0 11-6.75 0 3.375 3.375 0 016.75 0zm8.25 2.25a2.625 2.625 0 11-5.25 0 2.625 2.625 0 015.25 0z"
                          />
                        </svg>
                      </div>
                      <h2 className="text-lg font-bold mb-1">
                        You&apos;re not following anyone yet
                      </h2>
                      <p className="text-[var(--text-muted)] text-sm mb-6 max-w-xs mx-auto">
                        Follow creators you like to see their latest posts in your feed.
                      </p>
                      <Link
                        href="/explore"
                        className="inline-flex items-center gap-2 bg-violet-600 hover:bg-violet-500 text-white font-semibold text-sm px-5 py-2.5 rounded-xl transition-all shadow-md"
                      >
                        Find people to follow
                        <svg
                          className="w-4 h-4"
                          fill="none"
                          viewBox="0 0 24 24"
                          strokeWidth={2}
                          stroke="currentColor"
                          aria-hidden="true"
                        >
                          <path
                            strokeLinecap="round"
                            strokeLinejoin="round"
                            d="M13.5 4.5L21 12m0 0l-7.5 7.5M21 12H3"
                          />
                        </svg>
                      </Link>
                    </>
                  ) : (
                    <>
                      <div className="mx-auto mb-4 flex h-16 w-16 items-center justify-center rounded-full bg-[var(--bg-tertiary)]">
                        <svg
                          className="w-8 h-8 text-[var(--text-muted)]"
                          fill="none"
                          viewBox="0 0 24 24"
                          strokeWidth={1.5}
                          stroke="currentColor"
                          aria-hidden="true"
                        >
                          <path
                            strokeLinecap="round"
                            strokeLinejoin="round"
                            d="M12 7.5h1.5m-1.5 3h1.5m-7.5 3h7.5m-7.5 3h7.5m3-9h3.375c.621 0 1.125.504 1.125 1.125V18a2.25 2.25 0 01-2.25 2.25M16.5 7.5V18a2.25 2.25 0 002.25 2.25M16.5 7.5V4.875c0-.621-.504-1.125-1.125-1.125H4.125C3.504 3.75 3 4.254 3 4.875V18a2.25 2.25 0 002.25 2.25h13.5M6 7.5h3v3H6v-3z"
                          />
                        </svg>
                      </div>
                      <h2 className="text-lg font-bold mb-1">No posts found</h2>
                      <p className="text-[var(--text-muted)] text-sm mb-6">
                        {activeTab === "following"
                          ? "Accounts you follow haven't posted yet."
                          : "Be the first one to share something with the community!"}
                      </p>
                      {activeTab === "following" && (
                        <button
                          onClick={() => setActiveTab("explore")}
                          className="text-violet-400 hover:text-violet-300 font-semibold text-sm transition-colors"
                        >
                          Explore creators instead →
                        </button>
                      )}
                    </>
                  )}
                </div>
              ) : (
                /* Feed list */
                <>
                  <div className="space-y-4">
                    {posts.map((post) => (
                      <InteractivePostCard
                        key={post.id}
                        post={post}
                        currentUserAddress={currentUserAddress}
                        onTipClick={handleOpenTipModal}
                        userLikes={userLikes}
                      />
                    ))}
                  </div>

                  {/* Infinite Scroll Sentinel / Loading More */}
                  {hasMore && (
                    <div ref={sentinelRef} className="py-6 text-center">
                      {loadingMore ? (
                        <div className="flex items-center justify-center gap-2">
                          <Spinner />
                          <span className="text-sm text-[var(--text-muted)]">Loading more posts…</span>
                        </div>
                      ) : null}
                    </div>
                  )}
                </>
              )}
            </>
          )}
        </div>

        {/* Scroll to Top FAB */}
        {showScrollTop && (
          <button
            onClick={scrollToTop}
            className="fixed bottom-8 right-8 w-12 h-12 bg-violet-600 hover:bg-violet-500 text-white rounded-full shadow-xl flex items-center justify-center transition-all z-40"
            aria-label="Scroll to top"
          >
            <svg className="w-6 h-6" fill="none" viewBox="0 0 24 24" strokeWidth={2} stroke="currentColor">
              <path strokeLinecap="round" strokeLinejoin="round" d="M4.5 15.75l7.5-7.5 7.5 7.5" />
            </svg>
          </button>
        )}

        {/* Tipping Dialog Modal */}
        {tippingPost && (
          <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm p-4">
            <div className="w-full max-w-md rounded-2xl border border-[var(--border)] bg-[var(--muted)] p-4 md:p-6 shadow-2xl animate-in fade-in zoom-in duration-200">
              <header className="flex items-center justify-between mb-4">
                <h2 className="text-xl font-bold">Tip Creator</h2>
                <button
                  onClick={handleCloseTipModal}
                  className="text-[var(--text-muted)] hover:text-[var(--foreground)] text-xl transition-colors"
                  aria-label="Close tip modal"
                >
                  ✕
                </button>
              </header>

              <form onSubmit={handleTipSubmit} className="space-y-4" noValidate>
                <div>
                  <label className="block text-sm font-medium mb-1.5" htmlFor="tip-token">
                    Token Address <span className="text-red-500">*</span>
                  </label>
                  <input
                    id="tip-token"
                    type="text"
                    placeholder="G..."
                    value={tipToken}
                    onChange={(e) => setTipToken(e.target.value)}
                    className={`w-full rounded-xl border bg-[var(--bg-primary)] px-3.5 py-2.5 text-sm font-mono focus:outline-none focus:ring-2 focus:ring-violet-500 ${
                      tipErrors.token ? "border-red-500" : "border-[var(--border)]"
                    }`}
                  />
                  <FieldError id="tip-token-error" message={tipErrors.token} />
                </div>

                <div>
                  <label className="block text-sm font-medium mb-1.5" htmlFor="tip-amount">
                    Amount <span className="text-red-500">*</span>
                  </label>
                  <input
                    id="tip-amount"
                    type="number"
                    step="any"
                    placeholder="0.00"
                    value={tipAmount}
                    onChange={(e) => setTipAmount(e.target.value)}
                    className={`w-full rounded-xl border bg-[var(--bg-primary)] px-3.5 py-2.5 text-sm focus:outline-none focus:ring-2 focus:ring-violet-500 ${
                      tipErrors.amount ? "border-red-500" : "border-[var(--border)]"
                    }`}
                  />
                  <FieldError id="tip-amount-error" message={tipErrors.amount} />
                </div>

                <div className="flex flex-col sm:flex-row gap-3 justify-end pt-3">
                  <button
                    type="button"
                    onClick={handleCloseTipModal}
                    className="px-4.5 py-2 rounded-xl border border-[var(--border)] text-sm font-semibold hover:bg-[var(--bg-secondary)] transition-colors"
                  >
                    Cancel
                  </button>
                  <button
                    type="submit"
                    disabled={tipSubmitting}
                    className="px-5 py-2 rounded-xl bg-violet-600 text-white text-sm font-semibold hover:bg-violet-500 transition-colors shadow-md disabled:opacity-50"
                  >
                    {tipSubmitting ? "Sending..." : "Send Tip"}
                  </button>
                </div>
              </form>
            </div>
          </div>
        )}
      </div>
    </OnboardingGuard>
  );
}

function Spinner() {
  return (
    <svg
      className="animate-spin h-5 w-5 text-[var(--text-muted)]"
      xmlns="http://www.w3.org/2000/svg"
      fill="none"
      viewBox="0 0 24 24"
    >
      <circle
        className="opacity-25"
        cx="12"
        cy="12"
        r="10"
        stroke="currentColor"
        strokeWidth="4"
      ></circle>
      <path
        className="opacity-75"
        fill="currentColor"
        d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"
      ></path>
    </svg>
  );
}
