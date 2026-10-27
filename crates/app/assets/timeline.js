window.usttTimeline = {
  draw(canvas, scene) {
    if (!canvas || !scene) return;
    const dpr = window.devicePixelRatio || 1;
    const rect = canvas.getBoundingClientRect();
    const w = Math.max(1, Math.round(rect.width));
    const h = Math.max(1, Math.round(rect.height));
    if (canvas.width !== w * dpr || canvas.height !== h * dpr) {
      canvas.width = w * dpr;
      canvas.height = h * dpr;
    }
    const ctx = canvas.getContext("2d");
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, w, h);
    ctx.fillStyle = scene.background || "#14171c";
    ctx.fillRect(0, 0, w, h);
    const mid = h * 0.52;
    const barW = Math.max(1, scene.barWidth || 1.5);
    if (scene.peaks) {
      ctx.fillStyle = scene.waveColor || "#5b6472";
      for (let i = 0; i < scene.peaks.length; i++) {
        const p = scene.peaks[i];
        const x = p.x * w;
        const top = mid - p.max * h * 0.42;
        const bottom = mid - p.min * h * 0.42;
        ctx.fillRect(x, top, barW, Math.max(1, bottom - top));
      }
    }
    if (scene.turns) {
      for (const t of scene.turns) {
        ctx.fillStyle = t.color;
        ctx.globalAlpha = t.active ? 0.55 : 0.32;
        ctx.fillRect(t.x0 * w, 0, Math.max(2, (t.x1 - t.x0) * w), h);
      }
      ctx.globalAlpha = 1;
    }
  },
  setPlayhead(id, fraction) {
    const el = document.getElementById(id);
    if (el) el.style.transform = `translateX(${Math.max(0, Math.min(1, fraction)) * 100}%)`;
  },
  scrollToRow(id) {
    const el = document.getElementById(id);
    if (el) el.scrollIntoView({ block: "center", behavior: "smooth" });
  }
};
