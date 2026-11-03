const menuButton = document.querySelector(".menu-toggle");
const mobileNav = document.querySelector(".mobile-nav");

if (menuButton && mobileNav) {
  menuButton.addEventListener("click", () => {
    const expanded = menuButton.getAttribute("aria-expanded") === "true";
    menuButton.setAttribute("aria-expanded", String(!expanded));
    menuButton.setAttribute("aria-label", expanded ? "Open navigation" : "Close navigation");
    mobileNav.classList.toggle("is-open", !expanded);
  });

  mobileNav.querySelectorAll("a").forEach((link) => {
    link.addEventListener("click", () => {
      menuButton.setAttribute("aria-expanded", "false");
      menuButton.setAttribute("aria-label", "Open navigation");
      mobileNav.classList.remove("is-open");
    });
  });
}

document.querySelectorAll("[data-year]").forEach((node) => {
  node.textContent = String(new Date().getFullYear());
});

async function loadRelease() {
  try {
    const response = await fetch("https://api.github.com/repos/ADevNamedDeLL/Feather/releases/latest", {
      headers: { Accept: "application/vnd.github+json" }
    });
    if (!response.ok) return;
    const release = await response.json();
    if (!release || !release.tag_name) return;
    const version = release.tag_name.replace(/^v/, "");
    document.querySelectorAll("[data-version]").forEach((node) => {
      node.textContent = "v" + version;
    });
    const name = document.querySelector("[data-release-name]");
    if (name) name.textContent = "Feather " + release.tag_name;
    const zipAsset = release.assets?.find((asset) => asset.name.toLowerCase().endsWith(".zip"));
    const appImageAsset = release.assets?.find((asset) => asset.name.toLowerCase().endsWith(".appimage"));
    const zipLink = document.querySelector("[data-release-zip]");
    const appImageLink = document.querySelector("[data-release-appimage]");
    if (zipLink && zipAsset) zipLink.href = zipAsset.browser_download_url;
    if (appImageLink && appImageAsset) appImageLink.href = appImageAsset.browser_download_url;
    const date = document.querySelector("[data-release-date]");
    if (date && release.published_at) {
      date.textContent = "Published " + new Intl.DateTimeFormat(undefined, {
        year: "numeric",
        month: "long",
        day: "numeric"
      }).format(new Date(release.published_at));
    }
  } catch {
    return;
  }
}

loadRelease();