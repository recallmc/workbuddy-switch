import type { SessionGroupDetail } from "@/lib/types";

/** 一键同步的界面状态：进行中 / 成功 / 失败都直接驱动详情面板的提示条。 */
export type QuickSyncStatus = "idle" | "running" | "success" | "error";

export interface QuickSyncState {
  status: QuickSyncStatus;
  message?: string;
}

export type QuickSyncPrecheck =
  | { kind: "ready"; targetAccountIds: string[] }
  | { kind: "latest" | "blocked"; message: string };

/**
 * 点击「一键同步」前的本地判定：不发请求就能拦掉无数据 / 无需同步的场景，
 * 避免把空转也显示成失败。真正的安全校验仍由后端批量同步接口复核。
 */
export function precheckQuickSync(
  detail: Pick<SessionGroupDetail, "members" | "safeSourceMemberId"> | null,
): QuickSyncPrecheck {
  if (!detail) return { kind: "blocked", message: "会话详情尚未加载完成，请稍后重试" };
  const active = detail.members.filter((member) => member.linkState === "active");
  if (active.length === 0) return { kind: "blocked", message: "关联组暂无可同步的有效会话" };
  if (!detail.safeSourceMemberId) return { kind: "blocked", message: "当前没有可安全读取的来源副本" };
  const behind = active.filter((member) => member.versionStatus === "behind");
  if (behind.length === 0) return { kind: "latest", message: "已是最新，无需同步" };
  return {
    kind: "ready",
    targetAccountIds: behind
      .map((member) => member.accountId)
      .filter((id): id is string => Boolean(id)),
  };
}

/** 把批量同步报告收敛成一句带状态的提示，供详情面板内联展示。 */
export function summarizeQuickSync(
  report: {
    synced?: readonly unknown[];
    skipped?: readonly unknown[];
    errors?: readonly { error: string }[];
    needsRecovery?: boolean;
  },
): { status: "success" | "error"; message: string } {
  const synced = report.synced?.length ?? 0;
  const skipped = report.skipped?.length ?? 0;
  const errors = report.errors ?? [];
  if (report.needsRecovery) {
    return { status: "error", message: "内容已写入但仍有待恢复项，请按提示处理后重试" };
  }
  if (errors.length > 0) {
    const first = errors[0]?.error?.trim();
    return {
      status: "error",
      message: `已同步 ${synced} 个副本，仍有 ${errors.length} 项失败${first ? `：${first}` : ""}`,
    };
  }
  if (synced > 0) return { status: "success", message: `已同步 ${synced} 个副本` };
  if (skipped > 0) return { status: "success", message: `已是最新，${skipped} 项无需写入` };
  return { status: "success", message: "已是最新，无需同步" };
}
