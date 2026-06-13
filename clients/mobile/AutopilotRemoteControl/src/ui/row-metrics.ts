export const fixedRowLabelHeight = (lineHeight: number, lines = 2): number => {
  if (!Number.isFinite(lineHeight) || !Number.isFinite(lines)) return 0
  if (lineHeight < 0 || lines < 0) return 0
  return lineHeight * lines
}
