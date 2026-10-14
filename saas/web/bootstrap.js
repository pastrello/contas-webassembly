try {
  const { default: init } = await import('/pkg/contas_web.js');
  await init();
} catch {
  document.getElementById('wasm-status').textContent = 'Falha ao carregar WebAssembly';
  document.getElementById('overall-status').textContent = 'Não foi possível iniciar o aplicativo. Atualize a página ou confira a instalação.';
}
