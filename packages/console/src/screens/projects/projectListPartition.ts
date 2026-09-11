export type ProjectOwnership = {
  userId: string;
};

/** 按创建者拆分：我的项目 / 参与的项目 */
export function partitionProjectsByOwnership<T extends ProjectOwnership>(
  projects: T[],
  currentUserId: string | null | undefined,
): { mine: T[]; participating: T[] } {
  const mine: T[] = [];
  const participating: T[] = [];
  for (const project of projects) {
    if (currentUserId && project.userId === currentUserId) {
      mine.push(project);
    } else {
      participating.push(project);
    }
  }
  return { mine, participating };
}
