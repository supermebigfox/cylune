import {
  ArrowCounterClockwise,
  CheckCircle,
  Clock,
  Hourglass,
  Prohibit,
  StackSimple,
  WarningCircle,
} from "@phosphor-icons/react";
import { useEffect, useState } from "react";
import { Mark } from "../../brand/Mark";
import { Swatch } from "../../components/Swatch";
import { t, useLocale, type SupportedLocale } from "../../i18n";
import type {
  ImportPreview,
  JobOutcome,
  PlateStatus,
  PrintProjectDetail,
  SettlementResult,
  Spool,
  ToolMapping,
} from "../../lib/tauri";
import { Job } from "./Job";

export function ProjectMedia({
  alt,
  missingCopy,
  errorCopy,
  src,
}: {
  alt: string;
  missingCopy: string;
  errorCopy: string;
  src: string | null;
}) {
  const [failed, setFailed] = useState(false);

  useEffect(() => setFailed(false), [src]);

  if (src && !failed) {
    return (
      <img
        alt={alt}
        className="project-media-image"
        onError={() => setFailed(true)}
        src={src}
      />
    );
  }

  return (
    <span className="project-media-fallback">
      <Mark label={t("brand.mark")} size={38} />
      <span>{failed ? errorCopy : missingCopy}</span>
    </span>
  );
}

export function formatDuration(
  seconds: number,
  locale: SupportedLocale,
): string {
  const total = Math.max(0, Math.floor(seconds));
  const hours = Math.floor(total / 3600);
  const minutes = Math.floor((total % 3600) / 60);
  const remainder = total % 60;
  const units = locale === "zh-TW"
    ? { hour: "小時", minute: "分鐘", second: "秒" }
    : locale === "zh-CN"
      ? { hour: "小时", minute: "分钟", second: "秒" }
      : { hour: "hr", minute: "min", second: "sec" };
  const parts: string[] = [];

  if (hours) parts.push(`${hours} ${units.hour}`);
  if (minutes || (hours && remainder)) parts.push(`${minutes} ${units.minute}`);
  if (remainder) parts.push(`${remainder} ${units.second}`);
  if (!parts.length) parts.push(`0 ${units.minute}`);
  return parts.join(" ");
}

export function formatImportedAt(
  value: string,
  locale: SupportedLocale,
): string {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return new Intl.DateTimeFormat(locale, {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(date);
}

const statusIcons = {
  pending_mapping: Hourglass,
  ready: Hourglass,
  success: CheckCircle,
  failed: WarningCircle,
  cancelled: Prohibit,
  estimated: WarningCircle,
  skipped: Prohibit,
} satisfies Record<PlateStatus, typeof CheckCircle>;

const pendingStatuses = new Set<PlateStatus>(["pending_mapping", "ready"]);

function PlateStatusLabel({ status }: { status: PlateStatus }) {
  const locale = useLocale();
  const Icon = statusIcons[status];
  return (
    <span className={`plate-status status-${status}`}>
      <Icon aria-hidden="true" size={14} weight="fill" />
      {t(`project.status.${status}`, {}, locale)}
    </span>
  );
}

export function Project({
  project,
  selectedPlateId,
  preview = null,
  spools,
  initialMappings,
  result = null,
  busy = false,
  onSelectPlate,
  onConfirmMapping,
  onSettle,
  onConfirmNewPrint,
  onDiscard,
  onReverse,
}: {
  project: PrintProjectDetail;
  selectedPlateId: string | null;
  preview?: { plateId: string; value: ImportPreview } | null;
  spools: Spool[];
  initialMappings?: Record<number, string>;
  result?: { plateId: string; value: SettlementResult } | null;
  busy?: boolean;
  onSelectPlate(plateId: string): void;
  onConfirmMapping(jobId: string, mappings: ToolMapping[]): boolean | void | Promise<boolean | void>;
  onSettle(jobId: string, outcome: JobOutcome): boolean | void | Promise<boolean | void>;
  onConfirmNewPrint(sourceHash: string): boolean | void | Promise<boolean | void>;
  onDiscard?(jobId: string): boolean | void | Promise<boolean | void>;
  onReverse(jobId: string): boolean | void | Promise<boolean | void>;
}) {
  const locale = useLocale();
  const copy = (key: string, values: Record<string, string | number> = {}) =>
    t(key, values, locale);
  const processed = project.plates.filter(
    (plate) => !pendingStatuses.has(plate.status),
  ).length;
  const selectedPlate = project.plates.find(
    (plate) => plate.plate_id === selectedPlateId,
  );
  const selectedPreview = preview?.plateId === selectedPlateId
    ? preview.value
    : null;
  const selectedResult = result?.plateId === selectedPlateId
    ? result.value
    : null;

  return (
    <section className="page project-page" aria-labelledby="project-title">
      <header className="project-header">
        <div>
          <h1 id="project-title">{project.source_file_name}</h1>
          <p>
            {copy("history.importedAt", {
              date: formatImportedAt(project.imported_at, locale),
            })}
          </p>
        </div>
        <div className="project-progress">
          <span>{copy("project.totalProgress")}</span>
          <strong>{copy("project.processed", {
            count: processed,
            total: project.plate_count,
          })}</strong>
          <progress
            aria-label={copy("project.totalProgress")}
            max={project.plate_count}
            value={processed}
          />
        </div>
      </header>

      <section className="project-plates" aria-labelledby="project-plates-title">
        <div className="section-heading">
          <div>
            <h2 id="project-plates-title">{copy("project.plates")}</h2>
          </div>
        </div>
        <div className="plate-grid">
          {project.plates.map((plate) => {
            const selected = plate.plate_id === selectedPlateId;
            return (
              <button
                aria-pressed={selected}
                className={`plate-card ${selected ? "selected" : ""}`}
                key={plate.plate_id}
                onClick={() => onSelectPlate(plate.plate_id)}
                type="button"
              >
                <span className="sr-only">
                  {copy("project.openPlate", { number: plate.plate_index })}
                </span>
                <span className="plate-media">
                  <ProjectMedia
                    alt={copy("project.plateImage", {
                      number: plate.plate_index,
                    })}
                    errorCopy={copy("history.mediaError")}
                    missingCopy={copy("history.mediaMissing")}
                    src={plate.thumbnail_url}
                  />
                </span>
                <span className="plate-card-body">
                  <span className="plate-card-heading">
                    <strong>
                      {plate.display_name ??
                        copy("project.plateLabel", {
                          number: plate.plate_index,
                        })}
                    </strong>
                    <PlateStatusLabel status={plate.status} />
                  </span>
                  <span className="plate-facts">
                    <span>
                      <Clock aria-hidden="true" size={15} />
                      {plate.estimated_seconds == null
                        ? copy("history.unknownDuration")
                        : formatDuration(plate.estimated_seconds, locale)}
                    </span>
                    <span>
                      <StackSimple aria-hidden="true" size={15} />
                      {copy("project.layers", { count: plate.max_layer })}
                    </span>
                  </span>
                  <span className="plate-filaments">
                    {plate.filaments.map((filament) => (
                      <span className="plate-filament" key={filament.profile.tool}>
                        <span
                          aria-label={copy("project.colorSwatch", {
                            color: filament.profile.color_hex,
                          })}
                          className="plate-color-swatch"
                          role="img"
                        >
                          <Swatch colors={[filament.profile.color_hex]} />
                        </span>
                        <span className="data">
                          {filament.total_grams.toFixed(1)} {copy("common.grams")}
                        </span>
                      </span>
                    ))}
                    <strong className="plate-total data">
                      {copy("project.plateTotal", {
                        grams: plate.filaments
                          .reduce((sum, filament) => sum + filament.total_grams, 0)
                          .toFixed(1),
                      })}
                    </strong>
                  </span>
                </span>
              </button>
            );
          })}
        </div>
      </section>

      {selectedPlate ? (
        <section
          aria-label={copy("project.plateDetail", {
            number: selectedPlate.plate_index,
          })}
          className="plate-detail"
        >
          <div className="plate-detail-heading">
            <div>
              <h2>{selectedPlate.display_name ?? copy("project.plateLabel", {
                number: selectedPlate.plate_index,
              })}</h2>
              <PlateStatusLabel status={selectedPlate.status} />
            </div>
          </div>

          {pendingStatuses.has(selectedPlate.status) ? (
            selectedPreview ? (
              <>
                <div className="plate-filament-summary" aria-label={copy("project.filamentSummary")}>
                  {selectedPreview.filaments.map((filament) => (
                    <span key={filament.tool}>
                      <Swatch colors={[filament.profile.color_hex]} />
                      <span>
                        <strong className="data">
                          {filament.total_grams.toFixed(1)} {copy("common.grams")}
                        </strong>
                        <small className="data">{filament.profile.color_hex}</small>
                      </span>
                    </span>
                  ))}
                </div>
                <Job
                  busy={busy}
                  embedded
                  initialMappings={initialMappings}
                  onConfirmMapping={onConfirmMapping}
                  onConfirmNewPrint={onConfirmNewPrint}
                  onDiscard={onDiscard}
                  onReverse={onReverse}
                  onSettle={onSettle}
                  preview={selectedPreview}
                  spools={spools}
                />
              </>
            ) : (
              <div className="plate-detail-empty" role="status">
                <Hourglass aria-hidden="true" size={24} weight="duotone" />
                <span>{copy("project.preparing")}</span>
              </div>
            )
          ) : selectedPlate.status === "skipped" ? (
            <div className="plate-result skipped">
              <Prohibit aria-hidden="true" size={22} weight="fill" />
              <div>
                <strong>{copy("project.skippedTitle")}</strong>
                <p>{copy("project.skippedNoDeduction")}</p>
              </div>
            </div>
          ) : !selectedResult ? (
            <div className="plate-detail-empty" role="status">
              <Hourglass aria-hidden="true" size={24} weight="duotone" />
              <span>{copy("project.loadingResult")}</span>
            </div>
          ) : (
            <div className="plate-result-layout">
              <div className={`plate-result ${selectedPlate.status === "estimated" ? "estimated" : ""}`}>
                {selectedPlate.status === "estimated" ? (
                  <WarningCircle aria-hidden="true" size={22} weight="fill" />
                ) : (
                  <CheckCircle aria-hidden="true" size={22} weight="fill" />
                )}
                <div>
                  <strong>
                    {copy(selectedPlate.status === "estimated"
                      ? "project.estimatedResult"
                      : "project.settledResult")}
                  </strong>
                  <p>
                    {copy("project.deducted", {
                      grams: selectedResult.consumption
                        .reduce((sum, item) => sum + item.grams, 0)
                        .toFixed(1),
                    })}
                  </p>
                </div>
              </div>
              <button
                className="ghost"
                disabled={busy}
                onClick={() => onReverse(selectedResult.job_id)}
                type="button"
              >
                <ArrowCounterClockwise aria-hidden="true" size={17} />
                {copy("jobs.reverse")}
              </button>
            </div>
          )}
        </section>
      ) : null}
    </section>
  );
}
