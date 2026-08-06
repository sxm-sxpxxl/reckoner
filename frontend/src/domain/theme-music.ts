/**
 * Фоновая тема со шпионскими нотками.
 *
 * Синтезируется на месте через Web Audio: ни одного звукового файла в проекте
 * нет и быть не может — темы шпионских фильмов принадлежат правообладателям.
 * Это своя последовательность, а «шпионским» её делает не мелодия, а гармония:
 * минор, хроматический ход баса и аккорд `Am(maj7)` — тот самый неустойчивый
 * призвук, на котором держится весь жанр. Гармония не охраняется, в отличие
 * от конкретной мелодии.
 */

/** Ля минор. Такт — 2 секунды, петля — 8 тактов. */
const BPM = 100
const BEAT = 60 / BPM
const BAR = BEAT * 4
const BARS = 4
const LOOP = BAR * BARS

interface Note {
  hz: number
  /** Смещение от начала петли, в секундах. */
  at: number
  hold: number
  gain: number
  type: OscillatorType
}

const A2 = 110.0
const B2 = 123.47
const C3 = 130.81
const D3 = 146.83
const E3 = 164.81
const F3 = 174.61
const G3S = 207.65
const A3 = 220.0
const C4 = 261.63
const E4 = 329.63
const G4S = 415.3
const A4 = 440.0
const B4 = 493.88

/** Басовый ход: ровные четверти с хроматическим подъёмом к тонике в конце. */
const BASS: number[][] = [
  [A2, A2, C3, A2],
  [D3, D3, F3, D3],
  [E3, E3, G3S, E3],
  [A2, B2, C3, E3],
]

/** Верхний голос: редкие длинные ноты, чтобы петля не мозолила слух. */
const LEAD: { hz: number; bar: number; beat: number; hold: number }[] = [
  { hz: E4, bar: 0, beat: 2, hold: BEAT * 1.5 },
  { hz: C4, bar: 1, beat: 2.5, hold: BEAT },
  { hz: B4, bar: 2, beat: 1, hold: BEAT * 0.75 },
  { hz: A4, bar: 2, beat: 2, hold: BEAT * 1.5 },
  { hz: G4S, bar: 3, beat: 2, hold: BEAT * 2 },
]

function buildLoop(): Note[] {
  const notes: Note[] = []

  BASS.forEach((bar, barIndex) => {
    bar.forEach((hz, beatIndex) => {
      notes.push({
        hz,
        at: barIndex * BAR + beatIndex * BEAT,
        hold: BEAT * 0.7,
        gain: 0.5,
        // Пила даёт «щипок», похожий на сурф-гитару шпионских тем.
        type: 'sawtooth',
      })
    })
  })

  for (const note of LEAD) {
    notes.push({
      hz: note.hz,
      at: note.bar * BAR + note.beat * BEAT,
      hold: note.hold,
      gain: 0.32,
      type: 'triangle',
    })
  }

  // Am(maj7) целым тактом — держит напряжение под басом.
  for (const hz of [A3, C4, E4, G4S]) {
    notes.push({ hz, at: 2 * BAR, hold: BAR * 0.9, gain: 0.1, type: 'sine' })
  }

  return notes
}

const LOOP_NOTES = buildLoop()

/** Насколько вперёд планируем ноты, чтобы `setInterval` не опаздывал. */
const LOOKAHEAD = 0.25
const TICK_MS = 60

let context: AudioContext | null = null
let master: GainNode | null = null
let timer: number | null = null
let nextLoopAt = 0

function scheduleLoop(from: number) {
  if (!context || !master) return

  for (const note of LOOP_NOTES) {
    const start = from + note.at
    const end = start + note.hold

    const oscillator = context.createOscillator()
    const envelope = context.createGain()

    oscillator.type = note.type
    oscillator.frequency.value = note.hz

    // Огибающая руками: без неё на краях ноты слышен щелчок.
    envelope.gain.setValueAtTime(0.0001, start)
    envelope.gain.exponentialRampToValueAtTime(note.gain, start + 0.02)
    envelope.gain.exponentialRampToValueAtTime(0.0001, end)

    oscillator.connect(envelope)
    envelope.connect(master)
    oscillator.start(start)
    oscillator.stop(end + 0.05)
  }
}

function tick() {
  if (!context) return

  // Планируем следующую петлю заранее — иначе на стыке будет слышен разрыв.
  if (context.currentTime + LOOKAHEAD >= nextLoopAt) {
    scheduleLoop(nextLoopAt)
    nextLoopAt += LOOP
  }
}

/**
 * Запускает тему. Возвращает `true`, если звук пошёл.
 *
 * `false` означает, что браузер не дал играть без действия пользователя — это
 * политика автовоспроизведения, обойти её из кода нельзя. Вызывающий в таком
 * случае пробует ещё раз на первом клике.
 */
export async function startTheme(): Promise<boolean> {
  if (context) return true

  const Ctor =
    window.AudioContext ?? (window as { webkitAudioContext?: typeof AudioContext }).webkitAudioContext

  if (!Ctor) return false

  const created = new Ctor()

  if (created.state === 'suspended') {
    try {
      await created.resume()
    } catch {
      await created.close()

      return false
    }
  }

  // `resume()` может завершиться без ошибки, но контекст останется
  // приостановленным — проверяем состояние, а не факт вызова.
  if (created.state !== 'running') {
    await created.close()

    return false
  }

  context = created
  master = created.createGain()
  // Тихо: это фон, под ним считают деньги.
  master.gain.value = 0.07
  master.connect(created.destination)

  nextLoopAt = created.currentTime + 0.1
  tick()
  timer = window.setInterval(tick, TICK_MS)

  return true
}

export function stopTheme(): void {
  if (timer !== null) {
    window.clearInterval(timer)
    timer = null
  }

  void context?.close()
  context = null
  master = null
}
