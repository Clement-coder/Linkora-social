import { useState, useEffect } from 'react';
import { fetchLinkPreview, type LinkPreviewMetadata } from '@/lib/linkPreview';

export interface UseLinkPreviewResult {
  /** Metadata for the link, or null if loading/failed */
  metadata: LinkPreviewMetadata | null;
  /** True while fetching */
  loading: boolean;
  /** Error message if fetch failed */
  error: string | null;
  /** True if the metadata is a placeholder due to failure */
  isPlaceholder: boolean;
}

/**
 * Hook to fetch and cache link preview metadata.
 * 
 * - Fetches real Open Graph/Twitter Card metadata from the target URL
 * - Returns placeholder on failure rather than fabricated data
 * - Caches results per URL to avoid redundant fetches
 * 
 * @param url - The URL to fetch preview for (null/undefined to skip)
 * @returns Preview state with metadata, loading, and error info
 * 
 * @example
 * ```tsx
 * function LinkCard({ url }: { url: string }) {
 *   const { metadata, loading, isPlaceholder } = useLinkPreview(url);
 * 
 *   if (loading) return <div>Loading preview...</div>;
 *   if (!metadata) return null;
 * 
 *   return (
 *     <a href={url} className={isPlaceholder ? 'placeholder' : ''}>
 *       {metadata.image && <img src={metadata.image} alt="" />}
 *       <h3>{metadata.title}</h3>
 *       <p>{metadata.description}</p>
 *     </a>
 *   );
 * }
 * ```
 */
export function useLinkPreview(url?: string | null): UseLinkPreviewResult {
  const [metadata, setMetadata] = useState<LinkPreviewMetadata | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    // Skip if no URL provided
    if (!url) {
      setMetadata(null);
      setLoading(false);
      setError(null);
      return;
    }

    // Check cache first (in-memory cache per component instance)
    const cached = previewCache.get(url);
    if (cached) {
      setMetadata(cached);
      setLoading(false);
      setError(null);
      return;
    }

    // Fetch preview
    setLoading(true);
    setError(null);

    fetchLinkPreview(url)
      .then((preview) => {
        setMetadata(preview);
        previewCache.set(url, preview);
        setLoading(false);
      })
      .catch((err) => {
        const errMsg = err instanceof Error ? err.message : 'Failed to fetch preview';
        setError(errMsg);
        setLoading(false);
        // Don't set metadata on error - let the UI handle the error state
      });
  }, [url]);

  return {
    metadata,
    loading,
    error,
    isPlaceholder: metadata?.isPlaceholder ?? false,
  };
}

// Simple in-memory cache (shared across all hook instances)
const previewCache = new Map<string, LinkPreviewMetadata>();

/**
 * Clear the preview cache (useful for testing or manual refresh).
 */
export function clearPreviewCache() {
  previewCache.clear();
}
