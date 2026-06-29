const internalAdjutantCommand = /(^|\s)@adjutant(?=$|\s)/g

export const userFacingCopy = (value: string): string =>
  value
    .replaceAll('Adjutant', 'Autopilot')
    .replace(internalAdjutantCommand, match =>
      match.replace('@adjutant', '@autopilot'),
    )
