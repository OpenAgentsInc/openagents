import { useAtomValue } from "@effect/atom-react";
import { memo, useEffect, useRef, useState } from "react";
import type { EditorService } from "../../../packages/editor-services/service";
import { editorAtoms } from "../../../packages/editor-services/state";
import {
  DEFAULT_SCENE_EFFECTS,
  sameSceneEffects,
  type SceneEffectsSettings,
} from "../../../packages/runtime/scene-effects.ts";
import { Button } from "./components/ui/button";
import "./effects-controls.css";

function Slider({
  label,
  value,
  min,
  max,
  step,
  display,
  onChange,
}: {
  label: string;
  value: number;
  min: number;
  max: number;
  step: number;
  display: string;
  onChange: (value: number) => void;
}) {
  return (
    <label className="effects-slider">
      <span>
        {label}
        <output>{display}</output>
      </span>
      <input
        type="range"
        min={min}
        max={max}
        step={step}
        value={value}
        aria-label={label}
        aria-valuetext={display}
        onChange={(event) => onChange(Number(event.currentTarget.value))}
      />
    </label>
  );
}

/** Small editor controls; the preview worker owns the wind, particles, and frame loop. */
export const EffectsControls = memo(function EffectsControls({
  service,
  disabled,
}: {
  service: EditorService;
  disabled: boolean;
}) {
  const accepted = useAtomValue(editorAtoms.sceneEffects);
  const receipt = useAtomValue(editorAtoms.receipt);
  const [open, setOpen] = useState(false);
  const [draft, setDraft] = useState(accepted);
  const [error, setError] = useState<string | null>(null);
  const queued = useRef<SceneEffectsSettings | undefined>(undefined);
  const scheduled = useRef<number | undefined>(undefined);
  const latest = useRef<
    { id?: string; settings: SceneEffectsSettings } | undefined
  >(undefined);
  const button = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    if (
      !latest.current ||
      sameSceneEffects(accepted, latest.current.settings)
    ) {
      setDraft(accepted);
      latest.current = undefined;
    }
  }, [accepted]);
  useEffect(() => {
    if (receipt && !receipt.ok && receipt.requestId === latest.current?.id) {
      setError(receipt.message);
      setDraft(accepted);
      latest.current = undefined;
    }
  }, [receipt, accepted]);
  useEffect(
    () => () => {
      if (scheduled.current !== undefined)
        cancelAnimationFrame(scheduled.current);
      scheduled.current = undefined;
      queued.current = undefined;
      latest.current = undefined;
    },
    [],
  );

  function flush() {
    if (scheduled.current !== undefined)
      cancelAnimationFrame(scheduled.current);
    scheduled.current = undefined;
    const next = queued.current;
    queued.current = undefined;
    if (!next) return;
    try {
      latest.current = { id: service.sceneEffects(next), settings: next };
    } catch (cause) {
      latest.current = undefined;
      setDraft(accepted);
      setError(
        cause instanceof Error ? cause.message : "Couldn't update effects.",
      );
    }
  }
  function update(settings: SceneEffectsSettings) {
    setDraft(settings);
    setError(null);
    queued.current = settings;
    latest.current = { settings };
    if (scheduled.current === undefined)
      scheduled.current = requestAnimationFrame(flush);
  }
  function close() {
    setOpen(false);
    button.current?.focus();
  }
  const direction = Math.round(
    (Math.atan2(draft.wind.direction[1], draft.wind.direction[0]) * 180) /
      Math.PI,
  );
  return (
    <section
      className="effects-controls"
      aria-label="Scene effects"
      onKeyDown={(event) => {
        event.stopPropagation();
        if (event.key === "Escape") {
          event.preventDefault();
          close();
        }
      }}
      onKeyUp={(event) => event.stopPropagation()}
    >
      <Button
        ref={button}
        type="button"
        variant="secondary"
        aria-expanded={open}
        aria-controls="scene-effects-panel"
        disabled={disabled}
        onClick={() => {
          setOpen(!open);
          service.send({
            type: "input",
            input: {
              forward: 0,
              right: 0,
              up: 0,
              lookX: 0,
              lookY: 0,
              sprint: false,
              jump: false,
            },
          });
        }}
      >
        Effects
      </Button>
      {open ? (
        <div id="scene-effects-panel" className="effects-panel">
          <header>
            <h2>Live effects</h2>
            <button type="button" aria-label="Close effects" onClick={close}>
              ×
            </button>
          </header>
          <div
            className="effects-parameters"
            role="group"
            aria-label="Effect parameters"
            tabIndex={0}
          >
            <fieldset disabled={disabled}>
              <legend>Wind</legend>
              <label className="effects-toggle">
                <input
                  type="checkbox"
                  checked={draft.wind.enabled}
                  onChange={(event) =>
                    update({
                      ...draft,
                      wind: {
                        ...draft.wind,
                        enabled: event.currentTarget.checked,
                      },
                    })
                  }
                />
                Wind enabled
              </label>
              <Slider
                label="Wind speed"
                value={draft.wind.speed}
                min={0}
                max={30}
                step={0.1}
                display={`${draft.wind.speed.toFixed(1)} m/s`}
                onChange={(speed) =>
                  update({ ...draft, wind: { ...draft.wind, speed } })
                }
              />
              <Slider
                label="Wind direction"
                value={direction}
                min={-180}
                max={180}
                step={1}
                display={`${direction}°`}
                onChange={(degrees) => {
                  const angle = (degrees * Math.PI) / 180;
                  update({
                    ...draft,
                    wind: {
                      ...draft.wind,
                      direction: [Math.cos(angle), Math.sin(angle)],
                    },
                  });
                }}
              />
              <Slider
                label="Gust strength"
                value={draft.wind.gustStrength}
                min={0}
                max={1}
                step={0.01}
                display={`${Math.round(draft.wind.gustStrength * 100)}%`}
                onChange={(gustStrength) =>
                  update({ ...draft, wind: { ...draft.wind, gustStrength } })
                }
              />
            </fieldset>
            <fieldset disabled={disabled}>
              <legend>Grass</legend>
              <Slider
                label="Grass height"
                value={draft.grass.bladeHeight}
                min={0.1}
                max={2}
                step={0.01}
                display={`${draft.grass.bladeHeight.toFixed(2)} m`}
                onChange={(bladeHeight) =>
                  update({ ...draft, grass: { ...draft.grass, bladeHeight } })
                }
              />
              <Slider
                label="Grass width"
                value={draft.grass.bladeWidth}
                min={0.02}
                max={0.3}
                step={0.005}
                display={`${(draft.grass.bladeWidth * 100).toFixed(1)} cm`}
                onChange={(bladeWidth) =>
                  update({ ...draft, grass: { ...draft.grass, bladeWidth } })
                }
              />
              <Slider
                label="Grass wind response"
                value={draft.grass.windStrength}
                min={0}
                max={2}
                step={0.05}
                display={`${draft.grass.windStrength.toFixed(2)}×`}
                onChange={(windStrength) =>
                  update({ ...draft, grass: { ...draft.grass, windStrength } })
                }
              />
            </fieldset>
            <fieldset disabled={disabled}>
              <legend>Meteor impact</legend>
              <Slider
                label="Impact strength"
                value={draft.impact.intensity}
                min={0}
                max={4}
                step={0.05}
                display={`${draft.impact.intensity.toFixed(2)}×`}
                onChange={(intensity) =>
                  update({ ...draft, impact: { ...draft.impact, intensity } })
                }
              />
              <Slider
                label="Smoke"
                value={draft.impact.smoke}
                min={0}
                max={1}
                step={0.01}
                display={`${Math.round(draft.impact.smoke * 100)}%`}
                onChange={(smoke) =>
                  update({ ...draft, impact: { ...draft.impact, smoke } })
                }
              />
              <Slider
                label="Sparks"
                value={draft.impact.sparksCount}
                min={0}
                max={24}
                step={1}
                display={String(draft.impact.sparksCount)}
                onChange={(sparksCount) =>
                  update({ ...draft, impact: { ...draft.impact, sparksCount } })
                }
              />
            </fieldset>
          </div>
          <footer aria-label="Effect actions">
            <Button
              type="button"
              variant="secondary"
              disabled={disabled}
              onClick={() => {
                try {
                  flush();
                  service.previewImpact();
                } catch (cause) {
                  setError(
                    cause instanceof Error
                      ? cause.message
                      : "Couldn't preview the impact.",
                  );
                }
              }}
            >
              Preview impact
            </Button>
            <Button
              type="button"
              variant="ghost"
              disabled={disabled}
              onClick={() => update(DEFAULT_SCENE_EFFECTS)}
            >
              Reset
            </Button>
          </footer>
          <p className="effects-caption">Preview only · No damage</p>
          {error ? (
            <p className="effects-error" role="alert">
              {error}
            </p>
          ) : null}
        </div>
      ) : null}
    </section>
  );
});
