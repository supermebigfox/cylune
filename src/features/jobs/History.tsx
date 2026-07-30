import {
  CheckCircle,
  Clock,
  FolderOpen,
  Hourglass,
  Stack,
} from "@phosphor-icons/react";
import { Mark } from "../../brand/Mark";
import { t, useLocale } from "../../i18n";
import type { PrintProjectSummary } from "../../lib/tauri";
import { formatDuration, formatImportedAt, ProjectMedia } from "./Project";

function ProjectCard({
  project,
  pending,
  onOpenProject,
}: {
  project: PrintProjectSummary;
  pending: boolean;
  onOpenProject(projectId: string): void;
}) {
  const locale = useLocale();
  const copy = (key: string, values: Record<string, string | number> = {}) =>
    t(key, values, locale);
  const importedAt = formatImportedAt(project.imported_at, locale);
  const duration = project.total_estimated_seconds == null
    ? copy("history.unknownDuration")
    : formatDuration(project.total_estimated_seconds, locale);

  return (
    <article className="history-card">
      <button
        className="history-card-button"
        onClick={() => onOpenProject(project.project_id)}
        type="button"
      >
        <span className="sr-only">
          {copy("history.openProject", { name: project.source_file_name })}
        </span>
        <span className="project-media">
          <ProjectMedia
            alt={copy("history.projectCover", {
              name: project.source_file_name,
            })}
            errorCopy={copy("history.mediaError")}
            missingCopy={copy("history.mediaMissing")}
            src={project.cover_url}
          />
        </span>
        <span className="history-card-body">
          <span className={`project-status ${pending ? "pending" : "complete"}`}>
            {pending ? (
              <Hourglass aria-hidden="true" size={16} weight="fill" />
            ) : (
              <CheckCircle aria-hidden="true" size={16} weight="fill" />
            )}
            {copy(pending ? "history.statusPending" : "history.statusComplete")}
          </span>
          <strong className="history-project-name">
            {project.source_file_name}
          </strong>
          <span className="history-card-meta">
            {project.plate_count > 1 ? (
              <span>
                <Stack aria-hidden="true" size={15} />
                {copy("history.plateCount", { count: project.plate_count })}
              </span>
            ) : null}
            <span>
              <Clock aria-hidden="true" size={15} />
              {duration}
            </span>
          </span>
          <time dateTime={project.imported_at}>
            {copy("history.importedAt", { date: importedAt })}
          </time>
        </span>
      </button>
    </article>
  );
}

function HistoryGroup({
  emptyCopy,
  heading,
  pending,
  projects,
  onOpenProject,
}: {
  emptyCopy: string;
  heading: string;
  pending: boolean;
  projects: PrintProjectSummary[];
  onOpenProject(projectId: string): void;
}) {
  return (
    <section className="history-group" aria-labelledby={`history-${pending ? "pending" : "settled"}`}>
      <div className="section-heading">
        <div>
          <h2 id={`history-${pending ? "pending" : "settled"}`}>{heading}</h2>
        </div>
        <span className="history-count">{projects.length}</span>
      </div>
      {projects.length ? (
        <div className="history-grid">
          {projects.map((project) => (
            <ProjectCard
              key={project.project_id}
              onOpenProject={onOpenProject}
              pending={pending}
              project={project}
            />
          ))}
        </div>
      ) : (
        <div className="history-empty">
          <Mark label={t("brand.mark")} size={32} />
          <span>{emptyCopy}</span>
        </div>
      )}
    </section>
  );
}

export function History({
  pending,
  history,
  onOpenProject,
}: {
  pending: PrintProjectSummary[];
  history: PrintProjectSummary[];
  onOpenProject(projectId: string): void;
}) {
  const locale = useLocale();
  const copy = (key: string) => t(key, {}, locale);

  return (
    <section className="page history-page" aria-labelledby="history-title">
      <div className="page-heading">
        <div>
          <h1 id="history-title">{copy("history.title")}</h1>
          <p>{copy("history.hint")}</p>
        </div>
        <FolderOpen aria-hidden="true" className="page-heading-icon" size={28} weight="duotone" />
      </div>
      <HistoryGroup
        emptyCopy={copy("history.emptyPending")}
        heading={copy("history.pending")}
        onOpenProject={onOpenProject}
        pending
        projects={pending}
      />
      <HistoryGroup
        emptyCopy={copy("history.emptyHistory")}
        heading={copy("history.history")}
        onOpenProject={onOpenProject}
        pending={false}
        projects={history}
      />
    </section>
  );
}
