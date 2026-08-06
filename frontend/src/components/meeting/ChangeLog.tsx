import type { LogRecord } from '../../api/types'
import { formatLogTime } from '../../domain/format'
import styles from './ChangeLog.module.css'

/** Последние события. Сколько их — решает сервер (по спеке 12), клиент
 *  не обрезает: иначе пришлось бы держать это число в двух местах. */
export default function ChangeLog({ log }: { log: LogRecord[] }) {
  if (log.length === 0) return null

  return (
    <section className={styles.section}>
      <h2 className={styles.title}>Что менялось</h2>
      <ul className={styles.list}>
        {log.map((record) => (
          <li key={record.id} className={styles.item}>
            <span className={styles.time}>{formatLogTime(record.createdAt)}</span>
            <span className={styles.text}>{record.text}</span>
          </li>
        ))}
      </ul>
    </section>
  )
}
