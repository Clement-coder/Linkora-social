import { OptimisticStore } from "./optimisticStore";

describe("OptimisticStore", () => {
  beforeEach(() => {
    OptimisticStore.setLikeState("user:1", { isLiked: false, likeCount: 0 });
    OptimisticStore.setTipState("1", { tipTotal: 0 });
    OptimisticStore.setFollowState("user:target", {
      isFollowing: false,
      followersCount: 0,
      followingCount: 0,
    });
  });

  it("stores and reads optimistic like, tip, and follow state", () => {
    OptimisticStore.setLikeState("user:1", { isLiked: true, likeCount: 7 });
    OptimisticStore.setTipState("1", { tipTotal: 25 });
    OptimisticStore.setFollowState("user:target", {
      isFollowing: true,
      followersCount: 9,
      followingCount: 4,
    });

    expect(OptimisticStore.getLikeState("user:1")).toEqual({ isLiked: true, likeCount: 7 });
    expect(OptimisticStore.getTipState("1")).toEqual({ tipTotal: 25 });
    expect(OptimisticStore.getFollowState("user:target")).toEqual({
      isFollowing: true,
      followersCount: 9,
      followingCount: 4,
    });
  });

  it("supports optimistic reconciliation and rollback for like and tip updates", () => {
    const likeKey = "user:post-2";
    const tipKey = "post-2";

    OptimisticStore.setLikeState(likeKey, { isLiked: true, likeCount: 11 });
    OptimisticStore.setTipState(tipKey, { tipTotal: 125 });

    OptimisticStore.setLikeState(likeKey, { isLiked: false, likeCount: 10 });
    OptimisticStore.setTipState(tipKey, { tipTotal: 100 });

    expect(OptimisticStore.getLikeState(likeKey)).toEqual({ isLiked: false, likeCount: 10 });
    expect(OptimisticStore.getTipState(tipKey)).toEqual({ tipTotal: 100 });
  });

  it("removes optimistic state when a post is deleted from the feed", () => {
    const key = "user:deleted-post";
    OptimisticStore.setLikeState(key, { isLiked: true, likeCount: 3 });
    OptimisticStore.setTipState("deleted-post", { tipTotal: 12 });

    OptimisticStore.setLikeState(key, { isLiked: false, likeCount: 0 });
    OptimisticStore.setTipState("deleted-post", { tipTotal: 0 });

    expect(OptimisticStore.getLikeState(key)).toEqual({ isLiked: false, likeCount: 0 });
    expect(OptimisticStore.getTipState("deleted-post")).toEqual({ tipTotal: 0 });
  });
});
