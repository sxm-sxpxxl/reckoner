import { useEffect, useMemo, useRef } from 'react'

import { pickPhrases } from '../../domain/phrases'
import styles from './FloatingPhrases.module.css'

/** Сколько фраз летает одновременно. */
const COUNT = 9

/** Пикселей в секунду. Медленно: это фон, а не заставка. */
const SPEED_MIN = 12
const SPEED_MAX = 28

interface Flyer {
  text: string
  x: number
  y: number
  dx: number
  dy: number
}

/**
 * Фоновый слой с летающими фразами.
 *
 * Двигаем через `requestAnimationFrame` и `transform`, а не через состояние
 * React: перерисовка пяти элементов шестьдесят раз в секунду означала бы
 * шестьдесят рендеров всего поддерева в секунду.
 */
export default function FloatingPhrases() {
  const layer = useRef<HTMLDivElement>(null)
  const nodes = useRef<(HTMLSpanElement | null)[]>([])
  const phrases = useMemo(() => pickPhrases(COUNT), [])

  useEffect(() => {
    // Движущийся фон — первое, что мешает при вестибулярных расстройствах
    // и дефиците внимания. Если человек попросил покой, слой просто стоит.
    if (window.matchMedia('(prefers-reduced-motion: reduce)').matches) return

    const random = (min: number, max: number) => min + Math.random() * (max - min)
    const angle = () => {
      const direction = random(0, Math.PI * 2)
      const speed = random(SPEED_MIN, SPEED_MAX)

      return { dx: Math.cos(direction) * speed, dy: Math.sin(direction) * speed }
    }

    // Верхняя граница — под шапкой. Шапка липкая и с размытием, и фраза,
    // заезжающая под неё, размазывалась и выглядела сбоем, а не задумкой.
    const topEdge = () => document.querySelector('header')?.offsetHeight ?? 0

    const flyers: Flyer[] = phrases.map((text) => ({
      text,
      x: random(0.05, 0.85) * window.innerWidth,
      y: topEdge() + random(0.05, 0.8) * (window.innerHeight - topEdge()),
      ...angle(),
    }))

    let frame = 0
    let previous = performance.now()

    const step = (now: number) => {
      // Шаг по времени, а не по кадру: иначе на экране 120 Гц фразы летели бы
      // вдвое быстрее, чем на 60.
      const delta = Math.min((now - previous) / 1000, 0.05)

      previous = now

      flyers.forEach((flyer, index) => {
        const node = nodes.current[index]

        if (!node) return

        const width = node.offsetWidth
        const height = node.offsetHeight

        flyer.x += flyer.dx * delta
        flyer.y += flyer.dy * delta

        // Отскок от краёв: разворачиваем скорость и возвращаем внутрь, иначе
        // фраза может залипнуть за границей и дрожать там.
        if (flyer.x <= 0) {
          flyer.x = 0
          flyer.dx = Math.abs(flyer.dx)
        } else if (flyer.x + width >= window.innerWidth) {
          flyer.x = window.innerWidth - width
          flyer.dx = -Math.abs(flyer.dx)
        }

        const top = topEdge()

        if (flyer.y <= top) {
          flyer.y = top
          flyer.dy = Math.abs(flyer.dy)
        } else if (flyer.y + height >= window.innerHeight) {
          flyer.y = window.innerHeight - height
          flyer.dy = -Math.abs(flyer.dy)
        }

        node.style.transform = `translate3d(${flyer.x}px, ${flyer.y}px, 0)`
      })

      frame = requestAnimationFrame(step)
    }

    frame = requestAnimationFrame(step)

    return () => cancelAnimationFrame(frame)
  }, [phrases])

  return (
    // `aria-hidden`: экранному диктору незачем зачитывать летающие шутки
    // посреди сумм.
    <div className={styles.layer} ref={layer} aria-hidden="true">
      {phrases.map((text, index) => (
        <span
          key={text}
          ref={(node) => {
            nodes.current[index] = node
          }}
          className={styles.phrase}
          style={{ fontSize: `${22 + ((index * 7) % 24)}px` }}
        >
          {text}
        </span>
      ))}
    </div>
  )
}
