document.documentElement.classList.add("js");

const menuButton = document.querySelector("[data-menu-toggle]");
const mobileMenu = document.querySelector("[data-mobile-menu]");
const header = document.querySelector("[data-site-header]");
const reducedMotionQuery = window.matchMedia("(prefers-reduced-motion: reduce)");

const ambientSky = document.createElement("div");
ambientSky.className = "ambient-sky";
ambientSky.setAttribute("aria-hidden", "true");
for (const className of [
  "ambient-orb ambient-orb-one",
  "ambient-orb ambient-orb-two",
  "ambient-orb ambient-orb-three",
  "ambient-stars"
]) {
  const layer = document.createElement("span");
  layer.className = className;
  ambientSky.append(layer);
}
document.body.prepend(ambientSky);

function closeMenu() {
  if (!(menuButton instanceof HTMLButtonElement) || !(mobileMenu instanceof HTMLElement)) return;
  menuButton.setAttribute("aria-expanded", "false");
  menuButton.setAttribute("aria-label", "Open navigation");
  mobileMenu.hidden = true;
  document.body.classList.remove("menu-open");
}

if (menuButton instanceof HTMLButtonElement && mobileMenu instanceof HTMLElement) {
  menuButton.addEventListener("click", () => {
    const isOpen = menuButton.getAttribute("aria-expanded") === "true";
    menuButton.setAttribute("aria-expanded", String(!isOpen));
    menuButton.setAttribute("aria-label", isOpen ? "Open navigation" : "Close navigation");
    mobileMenu.hidden = isOpen;
    document.body.classList.toggle("menu-open", !isOpen);
  });

  mobileMenu.querySelectorAll("a").forEach((link) => link.addEventListener("click", closeMenu));
  window.addEventListener("keydown", (event) => {
    if (event.key === "Escape") closeMenu();
  });
}

function updateHeader() {
  if (header instanceof HTMLElement) {
    header.classList.toggle("is-scrolled", window.scrollY > 12);
  }
}

updateHeader();
window.addEventListener("scroll", updateHeader, { passive: true });

document.querySelectorAll("[data-year]").forEach((element) => {
  element.textContent = String(new Date().getFullYear());
});

const reducedMotion = reducedMotionQuery.matches;
const revealItems = document.querySelectorAll("[data-reveal]");

revealItems.forEach((element, index) => {
  element.style.setProperty("--reveal-order", String(index % 4));
});

if (reducedMotion || !("IntersectionObserver" in window)) {
  revealItems.forEach((element) => element.classList.add("is-visible"));
} else {
  const observer = new IntersectionObserver(
    (entries) => {
      entries.forEach((entry) => {
        if (!entry.isIntersecting) return;
        entry.target.classList.add("is-visible");
        observer.unobserve(entry.target);
      });
    },
    { rootMargin: "0px 0px -8% 0px", threshold: 0.1 }
  );

  revealItems.forEach((element) => observer.observe(element));
}

if (!reducedMotion) {
  const productStage = document.querySelector(".product-stage");

  if (productStage instanceof HTMLElement) {
    productStage.addEventListener("pointermove", (event) => {
      const bounds = productStage.getBoundingClientRect();
      const x = (event.clientX - bounds.left) / bounds.width - 0.5;
      const y = (event.clientY - bounds.top) / bounds.height - 0.5;
      productStage.style.setProperty("--tilt-x", `${(-y * 5).toFixed(2)}deg`);
      productStage.style.setProperty("--tilt-y", `${(x * 7).toFixed(2)}deg`);
      productStage.style.setProperty("--stage-x", `${((x + 0.5) * 100).toFixed(1)}%`);
      productStage.style.setProperty("--stage-y", `${((y + 0.5) * 100).toFixed(1)}%`);
    });

    productStage.addEventListener("pointerleave", () => {
      productStage.style.removeProperty("--tilt-x");
      productStage.style.removeProperty("--tilt-y");
      productStage.style.removeProperty("--stage-x");
      productStage.style.removeProperty("--stage-y");
    });
  }

  const interactiveCards = document.querySelectorAll(
    ".step-card, .feature-card, .release-card, .principles-grid article, .check-grid article, .instruction-list li"
  );

  interactiveCards.forEach((card) => {
    if (!(card instanceof HTMLElement)) return;
    card.addEventListener("pointermove", (event) => {
      const bounds = card.getBoundingClientRect();
      card.style.setProperty("--spot-x", `${event.clientX - bounds.left}px`);
      card.style.setProperty("--spot-y", `${event.clientY - bounds.top}px`);
    });
  });

  window.addEventListener(
    "pointermove",
    (event) => {
      document.documentElement.style.setProperty("--cursor-x", `${event.clientX}px`);
      document.documentElement.style.setProperty("--cursor-y", `${event.clientY}px`);
    },
    { passive: true }
  );
}
