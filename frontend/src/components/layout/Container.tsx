import type { ReactNode } from 'react'
import styles from './Container.module.css'

/** Ширина страницы задаётся в одном месте: 1180 px и поля по 28 px. */
export default function Container({ children }: { children: ReactNode }) {
  return <div className={styles.container}>{children}</div>
}
