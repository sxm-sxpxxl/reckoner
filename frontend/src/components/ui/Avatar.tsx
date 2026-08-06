import styles from './Avatar.module.css'

/** Палитра из восьми цветов; `colorIndex` уже приведён базой к 0..7. */
function paletteColor(colorIndex: number): string {
  return `var(--avatar-${((colorIndex % 8) + 8) % 8})`
}

interface AvatarProps {
  emoji: string
  colorIndex: number
  /** Размеры из хендоффа: 26, 28, 30, 36, 38, 42. */
  size: number
  name?: string
  /** Белая обводка — для наложенных стопкой аватаров. */
  ringed?: boolean
}

export default function Avatar({ emoji, colorIndex, size, name, ringed = false }: AvatarProps) {
  return (
    <span
      className={ringed ? `${styles.avatar} ${styles.ringed}` : styles.avatar}
      style={{
        width: size,
        height: size,
        background: paletteColor(colorIndex),
        // Эмодзи занимает примерно половину кружка — та же пропорция, что
        // в хендоффе: аватар 42 px, эмодзи 21 px.
        fontSize: Math.round(size * 0.5),
      }}
      title={name}
      role="img"
      aria-label={name ?? ''}
    >
      {emoji}
    </span>
  )
}
