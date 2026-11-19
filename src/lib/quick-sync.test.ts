import { describe, expect, it } from "vitest";

import { precheckQuickSync, summarizeQuickSync } from "./quick-sync";
import type { SessionGroupDetail } from "@/lib/types";

function member(overrides: Partial<SessionGroupDetail["members"][number]> = {}) {
  return {
    memberId: "m1",
    accountId: "acct-1",
    accountName: "账号一",
    uid: "uid-1",
    variant: "cn",
    linkState: "active",
    versionStatus: "behind",
    ...overrides,
  } as SessionGroupDetail["members"][number];
}

function detail(overrides: Partial<SessionGroupDetail> = {}): SessionGroupDetail {
  return {
    members: [member({ memberId: "src", versionStatus: "latest" }), member()],
    safeSourceMemberId: "src",
    ...overrides,
  } as SessionGroupDetail;
}

describe("precheckQuickSync", () => {
  it("详情未加载时给出明确提示", () => {
    expect(precheckQuickSync(null)).toEqual({ kind: "blocked", message: "会话详情尚未加载完成，请稍后重试" });
  });

  it("没有有效成员时不发起同步", () => {
    expect(precheckQuickSync(detail({ members: [member({ linkState: "stale" })] }))).toMatchObject({ kind: "blocked" });
  });

  it("没有安全来源时不发起同步", () => {
    expect(precheckQuickSync(detail({ safeSourceMemberId: undefined }))).toMatchObject({ kind: "blocked" });
  });

  it("全部一致时提示无需同步", () => {
    const value = precheckQuickSync(detail({ members: [member({ versionStatus: "latest" })] }));
    expect(value).toEqual({ kind: "latest", message: "已是最新，无需同步" });
  });

  it("存在落后副本时返回待同步账号", () => {
    const value = precheckQuickSync(
      detail({
        members: [member({ memberId: "a", accountId: "acct-a", versionStatus: "latest" }), member({ memberId: "b", accountId: "acct-b" }), member({ memberId: "c", accountId: undefined })],
      }),
    );
    expect(value).toEqual({ kind: "ready", targetAccountIds: ["acct-b"] });
  });
});

describe("summarizeQuickSync", () => {
  it("全部成功时报告数量", () => {
    expect(summarizeQuickSync({ synced: [{}, {}], skipped: [], errors: [] })).toEqual({
      status: "success",
      message: "已同步 2 个副本",
    });
  });

  it("只有跳过项时按最新处理", () => {
    expect(summarizeQuickSync({ synced: [], skipped: [{}], errors: [] })).toEqual({
      status: "success",
      message: "已是最新，1 项无需写入",
    });
  });

  it("没有变化时提示无需同步", () => {
    expect(summarizeQuickSync({ synced: [], skipped: [], errors: [] })).toEqual({
      status: "success",
      message: "已是最新，无需同步",
    });
  });

  it("部分失败时保留成功数量并给出首条错误", () => {
    expect(summarizeQuickSync({ synced: [{}], skipped: [], errors: [{ error: "预览已过期" }] })).toEqual({
      status: "error",
      message: "已同步 1 个副本，仍有 1 项失败：预览已过期",
    });
  });

  it("待恢复优先按失败提示", () => {
    expect(summarizeQuickSync({ synced: [{}], skipped: [], errors: [], needsRecovery: true })).toEqual({
      status: "error",
      message: "内容已写入但仍有待恢复项，请按提示处理后重试",
    });
  });
});
