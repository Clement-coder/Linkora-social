'use client';

import { useState } from 'react';

export interface Post {
  id: string;
  author: string;
  content: string;
  tip_total: string;
  timestamp: string;
  likes?: number;
}

export interface FeedProps {
  posts: Post[];
  loading?: boolean;
  isPaused?: boolean;
  onLike?: (postId: string) => Promise<void>;
  onTip?: (postId: string, amount: string) => Promise<void>;
}

export function Feed({
  posts,
  loading = false,
  isPaused = false,
  onLike,
  onTip,
}: FeedProps) {
  const [actionError, setActionError] = useState<string | null>(null);

  const executeGuardedWrite = async (action: () => Promise<void>) => {
    if (isPaused) {
      setActionError('Contract interaction is currently paused');
      return;
    }
    setActionError(null);
    try {
      await action();
    } catch (err) {
      setActionError(err instanceof Error ? err.message : 'Action failed');
    }
  };

  const handleLike = (postId: string) => {
    if (!onLike) return;
    executeGuardedWrite(() => onLike(postId));
  };

  const handleTip = (postId: string, amount: string) => {
    if (!onTip) return;
    executeGuardedWrite(() => onTip(postId, amount));
  };

  if (loading) {
    return (
      <div className="space-y-4 max-w-xl mx-auto py-4">
        {[1, 2, 3].map((i) => (
          <div key={i} className="bg-gray-100 animate-pulse h-32 rounded-lg" />
        ))}
      </div>
    );
  }

  if (posts.length === 0) {
    return (
      <div className="text-center py-12 text-gray-500">
        <p className="text-lg font-medium">No posts available</p>
      </div>
    );
  }

  return (
    <div className="space-y-4 max-w-xl mx-auto py-4">
      {actionError && (
        <div className="bg-red-50 border border-red-200 text-red-700 text-sm p-3 rounded-md mb-4">
          {actionError}
        </div>
      )}

      {posts.map((post) => (
        <div key={post.id} className="bg-white border border-gray-200 rounded-lg p-5 shadow-sm">
          <div className="flex justify-between items-start mb-2">
            <span className="text-sm font-semibold text-gray-800">@{post.author}</span>
            <span className="text-xs text-gray-500">{post.timestamp}</span>
          </div>
          <p className="text-gray-900 mb-4">{post.content}</p>
          <div className="flex items-center gap-4 text-sm text-gray-600">
            <button
              onClick={() => handleLike(post.id)}
              disabled={isPaused}
              className="hover:text-blue-600 disabled:opacity-50 font-medium"
            >
              Like ({post.likes || 0})
            </button>
            <button
              onClick={() => handleTip(post.id, '1.0')}
              disabled={isPaused}
              className="hover:text-green-600 disabled:opacity-50 font-medium"
            >
              Tip XLM ({post.tip_total})
            </button>
          </div>
        </div>
      ))}
    </div>
  );
}
