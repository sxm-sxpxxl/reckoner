/**
 * Подготовка обложки к отправке.
 *
 * Фотография с телефона — 3–5 МБ, и без уменьшения она не пройдёт ни лимит
 * сервера, ни здравый смысл: 0,5 ГБ Neon кончились бы на сотне встреч.
 * Поэтому сжатие обязательно, а не «на всякий случай».
 */

/** Больше этого по большей стороне не отправляем. */
const MAX_SIDE = 1600

/** Потолок сервера. Выше — гарантированный `413`, поэтому проверяем сами. */
const MAX_BYTES = 1024 * 1024

/** Качество JPEG: первая попытка и запасная, если результат не влез. */
const QUALITY_STEPS = [0.82, 0.7, 0.55]

export interface Size {
  width: number
  height: number
}

/**
 * Новые размеры с сохранением пропорций. Картинку меньше предела не
 * увеличиваем: она стала бы размытой и потяжелела бы без всякой пользы.
 */
export function fitWithin(width: number, height: number, max: number): Size {
  const longest = Math.max(width, height)

  if (longest <= max) return { width, height }

  const ratio = max / longest

  return {
    // `max(1, …)` не перестраховка: у панорамы вида 4000×10 короткая сторона
    // после округления вниз стала бы нулём, и canvas оказался бы невалидным.
    width: Math.max(1, Math.round(width * ratio)),
    height: Math.max(1, Math.round(height * ratio)),
  }
}

/** Не удалось подготовить файл. Текст показывается человеку как есть. */
export class ImageError extends Error {}

/**
 * Уменьшает и перекодирует картинку в JPEG.
 *
 * Качество понижается ступенями, пока результат не влезет в лимит. Если
 * не влез и на последней — бросаем: молча отправлять то, что сервер отобьёт,
 * значило бы показать человеку `413` вместо понятной причины.
 */
export async function compressImage(file: File): Promise<Blob> {
  let bitmap: ImageBitmap

  try {
    bitmap = await createImageBitmap(file)
  } catch {
    throw new ImageError('Не удалось прочитать файл — это точно картинка?')
  }

  const size = fitWithin(bitmap.width, bitmap.height, MAX_SIDE)
  const canvas = document.createElement('canvas')
  canvas.width = size.width
  canvas.height = size.height

  const context = canvas.getContext('2d')

  if (!context) {
    throw new ImageError('Браузер не дал обработать картинку')
  }

  context.drawImage(bitmap, 0, 0, size.width, size.height)
  bitmap.close()

  for (const quality of QUALITY_STEPS) {
    const blob = await new Promise<Blob | null>((resolve) => {
      canvas.toBlob(resolve, 'image/jpeg', quality)
    })

    if (!blob) {
      throw new ImageError('Не удалось сжать картинку')
    }

    if (blob.size <= MAX_BYTES) return blob
  }

  throw new ImageError('Картинка слишком большая даже после сжатия — попробуйте другую')
}
