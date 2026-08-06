import { formatRubles } from '../../domain/format'

/**
 * Денежное значение. Все суммы в дизайне моноширинные — отдельный компонент
 * не даёт об этом забыть и держит форматирование в одном месте.
 */
export default function Money({
  amount,
  size = 15,
  weight = 700,
  color,
}: {
  amount: number
  size?: number
  weight?: number
  color?: string
}) {
  return (
    <span style={{ fontFamily: 'var(--font-mono)', fontSize: size, fontWeight: weight, color }}>
      {formatRubles(amount)}
    </span>
  )
}
