// Dialog utility module for creating and managing dialogs

/**
 * 创建对话框
 * @param {Object} options
 * @param {string} options.title - 标题
 * @param {HTMLElement[]} options.content - 内容元素
 * @param {Object[]} options.buttons - 按钮配置
 * @param {string} options.buttons[].text - 按钮文本
 * @param {string} options.buttons[].className - 按钮样式类
 * @param {Function} options.buttons[].onClick - 点击回调
 * @returns {HTMLElement} overlay 元素
 */
window.createDialog = function ({ title, content = [], buttons = [] }) {
  const overlay = document.createElement("div");
  overlay.className = "dialog-overlay";

  const dialog = document.createElement("div");
  dialog.className = "rename-dialog";

  // 标题
  const titleEl = document.createElement("div");
  titleEl.className = "dialog-title";
  titleEl.textContent = title;
  dialog.appendChild(titleEl);

  // 内容
  for (const el of content) {
    dialog.appendChild(el);
  }

  // 按钮组
  if (buttons.length > 0) {
    const buttonsEl = document.createElement("div");
    buttonsEl.className = "dialog-buttons";
    for (const btn of buttons) {
      const btnEl = document.createElement("button");
      btnEl.className = `dialog-btn ${btn.className || ""}`;
      btnEl.textContent = btn.text;
      btnEl.addEventListener("click", btn.onClick);
      buttonsEl.appendChild(btnEl);
    }
    dialog.appendChild(buttonsEl);
  }

  overlay.appendChild(dialog);
  document.body.appendChild(overlay);

  // ESC 关闭
  overlay.addEventListener("keydown", (e) => {
    if (e.key === "Escape") overlay.remove();
  });

  return overlay;
};

/**
 * 关闭对话框
 */
window.closeDialog = function (overlay) {
  if (overlay && overlay.parentNode) {
    overlay.remove();
  }
};
