type ClassValue = string | number | boolean | null | undefined | Record<string, boolean> | ClassValue[]

export function cn(...inputs: ClassValue[]) {
  return inputs.flatMap(input => {
    if (!input) return []
    if (Array.isArray(input)) return cn(...input)
    if (typeof input === 'object') {
      return Object.entries(input)
        .filter(([, enabled]) => enabled)
        .map(([className]) => className)
    }
    return String(input)
  }).join(' ')
}
