// Svelte transitions with spring-like curves. They collapse to fades when the
// user prefers reduced motion.

import type { TransitionConfig } from "svelte/transition";

export function reducedMotion(): boolean {
  const root = document.documentElement;
  if (root.dataset.motion === "reduce") return true;
  return window.matchMedia?.("(prefers-reduced-motion: reduce)").matches ?? false;
}

/** Damped spring, normalised to settle at t = 1. */
export function springEase(t: number): number {
  const damping = 0.62;
  const omega = 11;
  return 1 - Math.exp(-damping * omega * t) * Math.cos(omega * Math.sqrt(1 - damping * damping) * t);
}

export function springIn(_node: Element, { delay = 0, y = 12 } = {}): TransitionConfig {
  if (reducedMotion()) return { delay, duration: 160, css: (t) => `opacity:${t}` };
  return {
    delay,
    duration: 560,
    css: (t) => {
      const s = springEase(t);
      const o = Math.min(1, t * 3);
      return `opacity:${o}; transform: translateY(${(1 - s) * y}px) scale(${0.97 + 0.03 * s})`;
    },
  };
}

export function collapseOut(node: Element): TransitionConfig {
  const h = (node as HTMLElement).offsetHeight;
  if (reducedMotion()) return { duration: 120, css: (t) => `opacity:${t}` };
  return {
    duration: 300,
    easing: (t) => t * t * (3 - 2 * t),
    css: (t) =>
      `opacity:${t}; transform: scale(${0.96 + 0.04 * t}); height:${t * h}px; margin-bottom:${(t - 1) * 10}px; overflow:hidden`,
  };
}

export function fadeScale(_node: Element, { duration = 220, start = 0.96 } = {}): TransitionConfig {
  if (reducedMotion()) return { duration: 120, css: (t) => `opacity:${t}` };
  return {
    duration,
    css: (t) => {
      const e = 1 - Math.pow(1 - t, 3);
      return `opacity:${e}; transform: scale(${start + (1 - start) * e})`;
    },
  };
}
