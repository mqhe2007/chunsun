import {
  onBeforeUnmount,
  onMounted,
  type MaybeRefOrGetter,
  toValue,
} from "vue";

/**
 * 营销页滚动揭示：优先 CSS scroll-driven（view timeline），
 * 不支持时回退 IntersectionObserver；尊重 prefers-reduced-motion。
 */
export function useLandingReveal(root: MaybeRefOrGetter<HTMLElement | null | undefined>) {
  let observer: IntersectionObserver | null = null;

  onMounted(() => {
    const el = toValue(root);
    if (!el) return;

    const reduced = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    if (reduced) {
      el.classList.add("landing-reveal-reduced");
      el.querySelectorAll<HTMLElement>("[data-reveal]").forEach((node) => {
        node.classList.add("is-revealed");
      });
      return;
    }

    const native =
      typeof CSS !== "undefined" &&
      CSS.supports("(animation-timeline: view())") &&
      CSS.supports("(animation-range: entry)");

    if (native) {
      el.classList.add("landing-reveal-native");
      return;
    }

    el.classList.add("landing-reveal-io");
    observer = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          if (!entry.isIntersecting) continue;
          entry.target.classList.add("is-revealed");
          observer?.unobserve(entry.target);
        }
      },
      { threshold: 0.18, rootMargin: "0px 0px -8% 0px" },
    );

    el.querySelectorAll<HTMLElement>("[data-reveal]").forEach((node) => {
      observer?.observe(node);
    });
  });

  onBeforeUnmount(() => {
    observer?.disconnect();
    observer = null;
  });
}
