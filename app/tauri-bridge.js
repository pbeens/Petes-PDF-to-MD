// Bridge to the app: provides window.pdfToMdApi, which renderer.js uses, on top
// of the Tauri commands in rust/app/src/main.rs and their events. (The API began
// as the Electron preload script's; see archive/electron-python/.)
(function () {
  const tauri = window.__TAURI__;
  if (!tauri || window.pdfToMdApi) return;
  const { invoke } = tauri.core;
  const { listen } = tauri.event;

  window.pdfToMdApi = {
    // Tauri gives dropped paths through its native drop event (below), not through File objects.
    getPathForFile: () => '',
    pickPdf: () => invoke('pick_pdf'),
    pickOutputDir: (currentPath) => invoke('pick_output_dir', { currentPath: currentPath || null }),
    getDefaultOutputRoot: () => invoke('get_default_output_root'),
    getAppMeta: () => invoke('get_app_meta'),
    openExternalUrl: (url) => invoke('open_external_url', { url }),
    validateInputPdf: (candidatePath) => invoke('validate_input_pdf', { candidatePath: candidatePath || '' }),
    resolveDroppedInputPdf: (payload) => invoke('resolve_dropped_input_pdf', { payload }),
    onConversionProgress: (handler) => {
      let unlisten = null;
      let cancelled = false;
      listen('conversion-progress', (event) => {
        if (typeof handler === 'function') handler(event.payload);
      }).then((fn) => {
        if (cancelled) fn();
        else unlisten = fn;
      });
      return () => {
        cancelled = true;
        if (unlisten) unlisten();
      };
    },
    runConversion: (inputPdfPath, outputRootPath, options) =>
      invoke('run_conversion', { inputPdfPath, outputRootPath: outputRootPath || null, options: options || {} }),
    loadOutline: (inputPdfPath, outputRootPath) =>
      invoke('load_outline', { inputPdfPath, outputRootPath: outputRootPath || null }),
    loadSection: (inputPdfPath, outputRootPath, sectionRelativePath) =>
      invoke('load_section', { inputPdfPath, outputRootPath: outputRootPath || null, sectionRelativePath }),
    openSectionInFolder: (inputPdfPath, outputRootPath, sectionRelativePath) =>
      invoke('open_section_in_folder', { inputPdfPath, outputRootPath: outputRootPath || null, sectionRelativePath }),
    openSectionDefault: (inputPdfPath, outputRootPath, sectionRelativePath) =>
      invoke('open_section_default', { inputPdfPath, outputRootPath: outputRootPath || null, sectionRelativePath }),
    copySectionPath: (inputPdfPath, outputRootPath, sectionRelativePath) =>
      invoke('copy_section_path', { inputPdfPath, outputRootPath: outputRootPath || null, sectionRelativePath }),
    openOutputDir: (inputPdfPath, outputRootPath) =>
      invoke('open_output_dir', { inputPdfPath, outputRootPath: outputRootPath || null }),
  };

  // Native file drops: the app sends `file-drag` events (see forward_file_drops
  // in rust/app/src/main.rs). A PDF dropped anywhere on the window is selected;
  // the Select PDF button lights up while a file is dragged over the window.
  listen('file-drag', (event) => {
    const { type, paths } = event.payload || {};
    const zone = document.getElementById('pickPdfBtn');
    if (zone) zone.classList.toggle('drag-active', type === 'enter');
    if (type === 'drop' && typeof window.pdfToMdHandleDroppedPaths === 'function') {
      window.pdfToMdHandleDroppedPaths(paths || []);
    }
  });
})();
