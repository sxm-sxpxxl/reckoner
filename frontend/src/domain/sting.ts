/**
 * Короткий приветственный сигнал.
 *
 * Синтезируется на месте через Web Audio: ни одного звукового файла в проекте
 * нет и быть не может — тема из шпионских фильмов принадлежит правообладателям,
 * а это своя последовательность из четырёх нот.
 *
 * Мотив: три коротких ноты по возрастанию и четвёртая, низкая и длинная, —
 * то самое «ту-ду-ду-тум».
 */

/** Ля минор: ноты в герцах и смещение от начала в секундах. */
const NOTES: { hz: number; at: number; hold: number }[] = [
  { hz: 440.0, at: 0, hold: 0.11 }, // A4  — ту
  { hz: 523.25, at: 0.13, hold: 0.11 }, // C5  — ду
  { hz: 659.25, at: 0.26, hold: 0.13 }, // E5  — ду
  { hz: 220.0, at: 0.42, hold: 0.55 }, // A3  — тум
]

const STORAGE_KEY = 'reckoner:sound'

/** Выключен ли звук. По умолчанию выключен: неожиданный звук на чужой странице
 *  раздражает сильнее, чем радует, — включается осознанно. */
export function isMuted(): boolean {
  try {
    return localStorage.getItem(STORAGE_KEY) !== 'on'
  } catch {
    // Приватный режим может запретить `localStorage`. Тогда считаем выключенным.
    return true
  }
}

export function setMuted(muted: boolean): void {
  try {
    localStorage.setItem(STORAGE_KEY, muted ? 'off' : 'on')
  } catch {
    // Не сохранилось — переживём, в этой сессии состояние всё равно в React.
  }
}

/**
 * Играет сигнал. Молча ничего не делает, если звук выключен или браузер
 * не дал контекст.
 *
 * Вызывать только из обработчика действия пользователя: до первого клика
 * браузеры не дают воспроизводить звук, и `AudioContext` остаётся `suspended`.
 */
export async function playSting(): Promise<void> {
  if (isMuted()) return

  const Ctor = window.AudioContext ?? (window as { webkitAudioContext?: typeof AudioContext }).webkitAudioContext

  if (!Ctor) return

  const context = new Ctor()

  // Если контекст создан вне жеста, он придёт `suspended`. Пробуем поднять;
  // не вышло — выходим тихо, без ошибок в консоли у человека.
  if (context.state === 'suspended') {
    try {
      await context.resume()
    } catch {
      await context.close()

      return
    }
  }

  const master = context.createGain()
  // Тихо: это подмигивание, а не заставка.
  master.gain.value = 0.16
  master.connect(context.destination)

  for (const note of NOTES) {
    const oscillator = context.createOscillator()
    const envelope = context.createGain()

    // Треугольник, а не пила: мягче и без металлического призвука.
    oscillator.type = 'triangle'
    oscillator.frequency.value = note.hz

    const start = context.currentTime + note.at
    const end = start + note.hold

    // Огибающая руками: без неё на включении и выключении слышен щелчок.
    envelope.gain.setValueAtTime(0.0001, start)
    envelope.gain.exponentialRampToValueAtTime(1, start + 0.012)
    envelope.gain.exponentialRampToValueAtTime(0.0001, end)

    oscillator.connect(envelope)
    envelope.connect(master)
    oscillator.start(start)
    oscillator.stop(end + 0.02)
  }

  // Контекст держит аудиоустройство занятым — закрываем, когда отзвучало.
  const total = NOTES[NOTES.length - 1].at + NOTES[NOTES.length - 1].hold + 0.1

  window.setTimeout(() => void context.close(), total * 1000)
}
