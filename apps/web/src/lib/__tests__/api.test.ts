import { fetchPools, fetchUserLikes } from "../api";

// Mock the global fetch
global.fetch = jest.fn();

describe("fetchPools", () => {
  beforeEach(() => {
    jest.clearAllMocks();
  });

  it("should throw an error when indexer returns non-ok status", async () => {
    (global.fetch as jest.Mock).mockResolvedValueOnce({
      ok: false,
      status: 500,
      statusText: "Internal Server Error",
    });

    await expect(fetchPools()).rejects.toThrow(
      "Indexer returned 500: Internal Server Error"
    );
  });

  it("should throw an error when network request fails", async () => {
    (global.fetch as jest.Mock).mockRejectedValueOnce(
      new Error("Network error")
    );

    await expect(fetchPools()).rejects.toThrow("Network error");
  });

  it("should return parsed pools on success", async () => {
    const mockResponse = {
      pools: [
        {
          pool_id: "test-pool",
          token: "GABC123",
          balance: "1000000",
          admins: ["GADMIN1", "GADMIN2"],
          threshold: 2,
        },
      ],
    };

    (global.fetch as jest.Mock).mockResolvedValueOnce({
      ok: true,
      json: async () => mockResponse,
    });

    const pools = await fetchPools();

    expect(pools).toHaveLength(1);
    expect(pools[0]).toEqual({
      id: "test-pool",
      token: "GABC123",
      balance: BigInt(1000000),
      adminCount: 2,
      threshold: 2,
    });
  });

  it("should handle array response format", async () => {
    const mockResponse = [
      {
        id: "test-pool-2",
        token: "GXYZ789",
        balance: "2000000",
        admin_count: 3,
        threshold: 2,
      },
    ];

    (global.fetch as jest.Mock).mockResolvedValueOnce({
      ok: true,
      json: async () => mockResponse,
    });

    const pools = await fetchPools();

    expect(pools).toHaveLength(1);
    expect(pools[0]).toEqual({
      id: "test-pool-2",
      token: "GXYZ789",
      balance: BigInt(2000000),
      adminCount: 3,
      threshold: 2,
    });
  });
});


describe("fetchUserLikes", () => {
  beforeEach(() => {
    jest.clearAllMocks();
  });

  it("should return a Set of liked post IDs", async () => {
    const mockResponse = {
      likes: ["1", "2", "3", "42"],
    };

    (global.fetch as jest.Mock).mockResolvedValueOnce({
      ok: true,
      json: async () => mockResponse,
    });

    const likes = await fetchUserLikes("GABC123");

    expect(likes).toBeInstanceOf(Set);
    expect(likes.size).toBe(4);
    expect(likes.has("1")).toBe(true);
    expect(likes.has("2")).toBe(true);
    expect(likes.has("3")).toBe(true);
    expect(likes.has("42")).toBe(true);
    expect(likes.has("5")).toBe(false);
  });

  it("should handle post_ids field name", async () => {
    const mockResponse = {
      post_ids: ["10", "20"],
    };

    (global.fetch as jest.Mock).mockResolvedValueOnce({
      ok: true,
      json: async () => mockResponse,
    });

    const likes = await fetchUserLikes("GDEF456");

    expect(likes.size).toBe(2);
    expect(likes.has("10")).toBe(true);
    expect(likes.has("20")).toBe(true);
  });

  it("should return empty Set when user has no likes", async () => {
    (global.fetch as jest.Mock).mockResolvedValueOnce({
      ok: true,
      json: async () => ({ likes: [] }),
    });

    const likes = await fetchUserLikes("GEMPTY");

    expect(likes.size).toBe(0);
  });

  it("should throw error on non-ok response", async () => {
    (global.fetch as jest.Mock).mockResolvedValueOnce({
      ok: false,
      status: 404,
      statusText: "Not Found",
    });

    await expect(fetchUserLikes("GBAD")).rejects.toThrow(
      "Failed to fetch likes: 404 Not Found"
    );
  });

  it("should throw error on network failure", async () => {
    (global.fetch as jest.Mock).mockRejectedValueOnce(
      new Error("Network error")
    );

    await expect(fetchUserLikes("GFAIL")).rejects.toThrow("Network error");
  });

  it("should convert numeric post IDs to strings", async () => {
    const mockResponse = {
      likes: [1, 2, 3], // Numeric IDs
    };

    (global.fetch as jest.Mock).mockResolvedValueOnce({
      ok: true,
      json: async () => mockResponse,
    });

    const likes = await fetchUserLikes("GABC");

    expect(likes.has("1")).toBe(true);
    expect(likes.has("2")).toBe(true);
    expect(likes.has("3")).toBe(true);
  });
});
