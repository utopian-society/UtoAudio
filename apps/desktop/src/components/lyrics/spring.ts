// This file is part of utoaudio, licensed under AGPL-3.0.
// Derivative work based on AMLL (https://github.com/amll-dev/applemusic-like-lyrics),
// which is also licensed under AGPL-3.0. See LICENSE for full license text.

/**
 * Closed-form analytical spring, ported from AMLL
 * `packages/core/src/utils/spring.ts`.
 *
 * Unlike a numerical spring, this solves the analytical second-order ODE
 * `mass * x'' + damping * x' + stiffness * (x - target) = 0` once at each
 * `setTargetPosition(...)` call, then integrates by evaluating the closed
 * form at the elapsed time. This gives a stable, frame-rate-independent
 * motion with no accumulation drift.
 */

export interface SpringParams {
  /** Mass term (higher → slower / more momentum). */
  mass: number;
  /** Damping coefficient (higher → less oscillation). */
  damping: number;
  /** Stiffness (higher → snappier). */
  stiffness: number;
  /** Whether to use a softer (critically-/over-damped exponential) solution. */
  soft?: boolean;
}

export const defaultPosYSpringParams: SpringParams = {
  mass: 0.9,
  damping: 15,
  stiffness: 90,
};

export const defaultScaleSpringParams: SpringParams = {
  mass: 2,
  damping: 25,
  stiffness: 100,
};

export const defaultBGSpringParams: SpringParams = {
  mass: 1,
  damping: 20,
  stiffness: 50,
};

/**
 * A closed-form spring that responds to {@link Spring.setTargetPosition}.
 * Ported from AMLL `packages/core/src/utils/spring.ts`.
 */
export class Spring {
  private params: SpringParams;
  private currentPos: number;
  private currentVel = 0;
  private target: number;
  private tAccum = 0;
  private delay = 0;
  private solver: (t: number) => number;

  constructor(initial = 0, params: SpringParams = defaultPosYSpringParams) {
    this.currentPos = initial;
    this.target = initial;
    this.params = params;
    this.solver = this.buildSolver(initial, initial, 0, params);
  }

  private buildSolver(
    from: number,
    to: number,
    vel0: number,
    params: SpringParams,
  ): (t: number) => number {
    const { mass, damping, stiffness, soft } = params;
    const delta = from - to;
    const angular = Math.sqrt(Math.max(0, stiffness / mass));
    const denom = 2 * Math.sqrt(Math.max(0, stiffness * mass));
    const overdamped = soft || damping >= denom;

    if (overdamped) {
      // Critically-damped / over-damped exponential decay.
      const zeta = damping / denom;
      const r = -angular * zeta;
      // x(t) = to + (delta + (vel0 - r*delta) * t) * e^(r t)
      const leftover = vel0 - r * delta;
      return (t: number) => to + (delta + leftover * t) * Math.exp(r * t);
    }

    // Under-damped: sinusoid × decaying exponential.
    const df = Math.sqrt(
      Math.max(1e-9, angular * angular - (damping / (2 * mass)) ** 2),
    );
    const dm = -damping / (2 * mass);
    return (t: number) =>
      to +
      (Math.cos(df * t) * delta +
        (Math.sin(df * t) * (vel0 - dm * delta)) / df) *
        Math.exp(dm * t);
  }

  /** Update the spring by `deltaSeconds`; returns current position. */
  update(deltaSeconds: number): number {
    if (this.delay > 0) {
      this.delay = Math.max(0, this.delay - deltaSeconds);
      return this.currentPos;
    }
    this.tAccum += deltaSeconds;
    const pos = this.solver(this.tAccum);
    const prev = this.currentPos;
    this.currentPos = pos;
    this.currentVel =
      deltaSeconds > 0 ? (pos - prev) / deltaSeconds : this.currentVel;
    return pos;
  }

  /** Current position. */
  getCurrentPosition(): number {
    return this.currentPos;
  }

  /** Current velocity (units/s). */
  getCurrentVelocity(): number {
    return this.currentVel;
  }

  /** Whether the spring has effectively settled on its target. */
  arrived(threshold = 0.01): boolean {
    return (
      Math.abs(this.target - this.currentPos) < threshold &&
      Math.abs(this.currentVel) < threshold &&
      this.delay <= 0
    );
  }

  /** Re-target, optionally after a `delaySeconds` wait (velocity preserved). */
  setTargetPosition(target: number, delaySeconds = 0): void {
    this.target = target;
    this.delay = delaySeconds;
    // Record velocity at the re-target instant for a seamless handoff.
    const v0 = this.currentVel;
    this.solver = this.buildSolver(this.currentPos, target, v0, this.params);
    this.tAccum = 0;
  }

  /** Jump to a position/velocity with no spring transition (useful for seeks). */
  setPositionInstant(position: number, velocity = 0): void {
    this.currentPos = position;
    this.currentVel = velocity;
    this.target = position;
    this.solver = () => position;
    this.tAccum = 0;
    this.delay = 0;
  }

  /** Replace spring params (next re-target picks them up). */
  setParameters(params: Partial<SpringParams>): void {
    this.params = { ...this.params, ...params };
    this.solver = this.buildSolver(
      this.currentPos,
      this.target,
      this.currentVel,
      this.params,
    );
    this.tAccum = 0;
  }
}
