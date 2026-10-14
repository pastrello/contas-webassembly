try {
  const { default: init } = await import('/pkg/contas_web.js');
  await init();
} catch {
  document.getElementById('status').textContent = 'Não foi possível carregar o aplicativo. Atualize a página ou confira a instalação.';
}
