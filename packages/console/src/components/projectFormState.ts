import type { Project } from "../types/project";

export type ProjectFormState = {
  name: string;
  description: string;
};

export function createProjectFormState(
  project?: Project | null,
): ProjectFormState {
  return {
    name: project?.name ?? "",
    description: project?.description ?? "",
  };
}

export function buildCreateProjectPayload(form: ProjectFormState) {
  return {
    name: form.name,
    description: form.description || undefined,
  };
}
