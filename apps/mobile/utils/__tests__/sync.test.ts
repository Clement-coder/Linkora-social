import {
  addOutboxDmMessage,
  getCachedPostsByIds,
  getDmSyncCursor,
  markDmMessageFailed,
  mergeDmDeltas,
  reconcilePosts,
  setDmSyncCursor,
} from "../db";
import {
  computeCiphertextHash,
  DmClient,
  DmSourceMessage,
  fetchAndCachePosts,
  reconcileDmThread,
  sendDmMessageWithOutbox,
} from "../sync";

jest.mock("../db", () => ({
  addOutboxDmMessage: jest.fn(),
  confirmPendingPost: jest.fn(),
  getCachedPostsByIds: jest.fn(),
  getDmSyncCursor: jest.fn(),
  getPendingPosts: jest.fn(),
  markDmMessageFailed: jest.fn(),
  markPendingPostFailed: jest.fn(),
  mergeDmDeltas: jest.fn(),
  reconcilePosts: jest.fn(),
  setDmSyncCursor: jest.fn(),
}));

const mockedGetDmSyncCursor = getDmSyncCursor as jest.Mock;
const mockedMergeDmDeltas = mergeDmDeltas as jest.Mock;
const mockedSetDmSyncCursor = setDmSyncCursor as jest.Mock;
const mockedAddOutboxDmMessage = addOutboxDmMessage as jest.Mock;
const mockedMarkDmMessageFailed = markDmMessageFailed as jest.Mock;
const mockedGetCachedPostsByIds = getCachedPostsByIds as jest.Mock;
const mockedReconcilePosts = reconcilePosts as jest.Mock;

function fakeClient(overrides: Partial<DmClient> = {}): DmClient {
  return {
    getMessages: jest.fn().mockResolvedValue([]),
    sendMessage: jest.fn().mockResolvedValue(undefined),
    ...overrides,
  };
}

beforeEach(() => {
  jest.clearAllMocks();
});

describe("computeCiphertextHash", () => {
  it("is deterministic for the same input", () => {
    expect(computeCiphertextHash("hello world")).toBe(computeCiphertextHash("hello world"));
  });

  it("differs for different content, so distinct messages never collide as duplicates", () => {
    expect(computeCiphertextHash("hello")).not.toBe(computeCiphertextHash("hell0"));
  });
});

describe("reconcileDmThread", () => {
  const conversationId = "convo-1";
  const otherAddress = "GRECIPIENT";

  it("fetches deltas strictly after the stored cursor and merges only those", async () => {
    mockedGetDmSyncCursor.mockResolvedValue(100);
    mockedMergeDmDeltas.mockResolvedValue({ mergedCount: 2, newestTimestamp: 150 });

    const source: DmSourceMessage[] = [
      { id: "m1", sender: "A", recipient: "B", content: "old", timestamp: 50 },
      { id: "m2", sender: "A", recipient: "B", content: "boundary", timestamp: 100 },
      { id: "m3", sender: "A", recipient: "B", content: "new-1", timestamp: 120 },
      { id: "m4", sender: "A", recipient: "B", content: "new-2", timestamp: 150 },
    ];
    const client = fakeClient({ getMessages: jest.fn().mockResolvedValue(source) });

    const result = await reconcileDmThread(client, conversationId, otherAddress);

    // The cursor value itself ("boundary") must NOT be re-merged — only messages
    // strictly newer than it. Mutating the filter to `>=` would break this.
    expect(mockedMergeDmDeltas).toHaveBeenCalledWith(conversationId, [
      expect.objectContaining({ id: "m3", timestamp: 120 }),
      expect.objectContaining({ id: "m4", timestamp: 150 }),
    ]);
    expect(mockedSetDmSyncCursor).toHaveBeenCalledWith(conversationId, 150);
    expect(result).toEqual({ mergedCount: 2, latestSyncedTimestamp: 150 });
  });

  it("computes the ciphertext hash from content when no ciphertext is present", async () => {
    mockedGetDmSyncCursor.mockResolvedValue(0);
    mockedMergeDmDeltas.mockResolvedValue({ mergedCount: 1, newestTimestamp: 10 });
    const client = fakeClient({
      getMessages: jest
        .fn()
        .mockResolvedValue([
          { id: "m1", sender: "A", recipient: "B", content: "hi", timestamp: 10 },
        ]),
    });

    await reconcileDmThread(client, conversationId, otherAddress);

    expect(mockedMergeDmDeltas).toHaveBeenCalledWith(conversationId, [
      expect.objectContaining({ ciphertextHash: computeCiphertextHash("hi") }),
    ]);
  });

  it("does nothing when there are no messages newer than the cursor, so reconnect never re-merges history", async () => {
    mockedGetDmSyncCursor.mockResolvedValue(200);
    const client = fakeClient({
      getMessages: jest
        .fn()
        .mockResolvedValue([
          { id: "m1", sender: "A", recipient: "B", content: "old", timestamp: 100 },
        ]),
    });

    const result = await reconcileDmThread(client, conversationId, otherAddress);

    expect(mockedMergeDmDeltas).not.toHaveBeenCalled();
    expect(mockedSetDmSyncCursor).not.toHaveBeenCalled();
    expect(result).toEqual({ mergedCount: 0, latestSyncedTimestamp: null });
  });
});

describe("sendDmMessageWithOutbox", () => {
  const conversationId = "convo-1";
  const sender = "GSENDER";
  const recipient = "GRECIPIENT";

  it("keeps the outbox entry pending and does not mark it failed when the relay accepts the send", async () => {
    const outboxMessage = {
      id: "dm_local_1",
      conversationId,
      sender,
      recipient,
      content: "hey",
      ciphertextHash: computeCiphertextHash("hey"),
      timestamp: 1000,
      syncStatus: "pending" as const,
      errorMessage: null,
    };
    mockedAddOutboxDmMessage.mockResolvedValue(outboxMessage);
    const client = fakeClient({ sendMessage: jest.fn().mockResolvedValue(undefined) });

    const result = await sendDmMessageWithOutbox(client, conversationId, sender, recipient, "hey");

    expect(client.sendMessage).toHaveBeenCalledWith(recipient, "hey");
    expect(mockedMarkDmMessageFailed).not.toHaveBeenCalled();
    expect(result).toEqual(outboxMessage);
  });

  it("surfaces a relay rejection as a failed outbox entry with the relay's error", async () => {
    const outboxMessage = {
      id: "dm_local_2",
      conversationId,
      sender,
      recipient,
      content: "hey",
      ciphertextHash: computeCiphertextHash("hey"),
      timestamp: 1000,
      syncStatus: "pending" as const,
      errorMessage: null,
    };
    mockedAddOutboxDmMessage.mockResolvedValue(outboxMessage);
    const client = fakeClient({
      sendMessage: jest.fn().mockRejectedValue(new Error("401 invalid signature")),
    });

    const result = await sendDmMessageWithOutbox(client, conversationId, sender, recipient, "hey");

    expect(mockedMarkDmMessageFailed).toHaveBeenCalledWith(
      outboxMessage.id,
      "401 invalid signature"
    );
    expect(result).toEqual({
      ...outboxMessage,
      syncStatus: "failed",
      errorMessage: "401 invalid signature",
    });
  });
});

describe("fetchAndCachePosts", () => {
  const originalFetch = global.fetch;

  afterEach(() => {
    global.fetch = originalFetch;
  });

  function mockIndexerResponse(posts: unknown[]) {
    global.fetch = jest.fn().mockResolvedValue({
      ok: true,
      json: jest.fn().mockResolvedValue({ posts }),
    }) as unknown as typeof fetch;
  }

  it("looks up the local cache in a single batched call instead of one per post", async () => {
    const indexerPosts = Array.from({ length: 20 }, (_, i) => ({
      id: i,
      author: `GAUTHOR${i}`,
      content: `content-${i}`,
      username: `user-${i}`,
      tip_total: 0,
      created_ledger: 1000,
      like_count: 0,
      has_liked: false,
    }));
    mockIndexerResponse(indexerPosts);
    mockedGetCachedPostsByIds.mockResolvedValue(new Map());

    await fetchAndCachePosts(20, 0);

    // A per-post lookup would call this 20 times; batching keeps it to one call
    // no matter how many posts the indexer returns.
    expect(mockedGetCachedPostsByIds).toHaveBeenCalledTimes(1);
    expect(mockedGetCachedPostsByIds).toHaveBeenCalledWith(indexerPosts.map((p) => String(p.id)));
  });

  it("prefers cached content/username over the indexer's fallback values", async () => {
    mockIndexerResponse([
      { id: "1", author: "GAUTHOR1", content: "raw indexer content", username: "raw_user" },
    ]);
    mockedGetCachedPostsByIds.mockResolvedValue(
      new Map([
        [
          "1",
          {
            id: "1",
            author: "GAUTHOR1",
            username: "cached_user",
            content: "cached content",
            tip_total: 5,
            timestamp: 500,
            like_count: 2,
            has_liked: true,
          },
        ],
      ])
    );

    const [post] = await fetchAndCachePosts(1, 0);

    expect(post).toMatchObject({ content: "cached content", username: "cached_user" });
  });

  it("falls back to indexer content/username and reconciles the fetched posts when nothing is cached", async () => {
    mockIndexerResponse([{ id: "2", author: "GAUTHOR2", content: "", username: "" }]);
    mockedGetCachedPostsByIds.mockResolvedValue(new Map());

    const [post] = await fetchAndCachePosts(1, 0);

    expect(post).toMatchObject({
      content: "Content unavailable offline",
      username: "GAUTHO...HOR2",
    });
    expect(mockedReconcilePosts).toHaveBeenCalledWith([post]);
  });

  it("throws and skips the cache lookup when the indexer request fails", async () => {
    global.fetch = jest.fn().mockResolvedValue({ ok: false }) as unknown as typeof fetch;

    await expect(fetchAndCachePosts(10, 0)).rejects.toThrow("Failed to fetch posts from indexer");
    expect(mockedGetCachedPostsByIds).not.toHaveBeenCalled();
  });
});
