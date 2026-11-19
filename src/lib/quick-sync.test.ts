import { describe, expect, it } from "vitest";

import { summarizeQuickSync } from "./quick-sync";

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
