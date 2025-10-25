declare module 'gradient-gl' {
  type GradientSeedTuple = [shaderId: string, uniforms: Uint8Array]

  export interface GradientProgram {
    shaderId: string
    init(): void
    updateSeed(seed: GradientSeedTuple): boolean
    destroy(): void
  }

  /**
   * Bootstraps the gradient canvas for the supplied seed and target selector.
   * Resolves with the active GradientProgram instance once initialized.
   */
  export default function gradientGL(seed: string, selector?: string): Promise<GradientProgram>
}
