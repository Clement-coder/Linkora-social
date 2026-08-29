"use client";

import { useNotificationsContext } from "@/contexts/NotificationsContext";

export type { NotificationType, Notification } from "@/contexts/NotificationsContext";

/**
 * Thin consumer over the canonical NotificationsProvider.
 *
 * All notification state (including the indexer WebSocket feed) now lives in
 * `NotificationsContext`, so this hook lets existing callers keep reading the
 * inbox without owning their own copy of the data.
 */
export function useNotifications() {
  const { notifications, hasMore, inboxUnreadCount, markAllRead, loadMore } =
    useNotificationsContext();

  return {
    notifications,
    hasMore,
    unreadCount: inboxUnreadCount,
    markAllRead,
    loadMore,
  };
}
