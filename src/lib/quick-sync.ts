/** 把批量同步报告收敛成一句带状态的提示，供按钮 title 与 toast 复用。 */
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
