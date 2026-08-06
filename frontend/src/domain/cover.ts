/** Градиенты-заглушки обложек из хендоффа. */
const GRADIENTS = [
  'linear-gradient(135deg,#F7C6A8,#E7A183)',
  'linear-gradient(135deg,#BFD9C6,#8FBBA1)',
  'linear-gradient(135deg,#C9CFEA,#9AA6D6)',
  'linear-gradient(135deg,#F0D69B,#DCB86A)',
]

/**
 * Градиент по идентификатору встречи.
 *
 * Хендофф предлагает брать индекс карточки в списке, но тогда при смене
 * сортировки у встречи меняется картинка — список моргает, и обложка перестаёт
 * быть приметой встречи. Хеш от `id` даёт стабильный цвет на всю её жизнь.
 */
export function coverGradient(id: string): string {
  let hash = 0

  for (let index = 0; index < id.length; index += 1) {
    hash = (hash * 31 + id.charCodeAt(index)) >>> 0
  }

  return GRADIENTS[hash % GRADIENTS.length]
}
