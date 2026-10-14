# Ciclo 4 — lançamentos e saldos

O workspace Rust está em 0.4.0, na branch codex/saas-cycle-4. A versão desktop C#/SQLite e suas releases permanecem separadas.

## Uso

1. Entre com o usuário existente e confira as contas e históricos cadastrados.
2. Abra **Lançamentos**, selecione a conta e clique **Consultar / atualizar saldos**.
3. Informe conta, data, histórico e valor positivo, sem separador de milhar. Documento e complemento são opcionais. O histórico determina Entrada ou Saída.
4. Para compensar, desfazer compensação, arquivar ou desarquivar, clique **Editar**, altere as opções e salve.
5. Para corrigir valor, data, conta ou histórico, edite e salve. A troca de conta retira o efeito da origem e o aplica ao destino em uma transação.
6. **Cancelar lançamento** exige confirmação e retira o valor dos saldos. O registro permanece consultável como cancelado e não pode ser editado nem reativado.
7. O período inclui as duas datas. Arquivados/cancelados têm filtros próprios. A consulta mostra até 200 registros por página, ordenados por data e sequência de inclusão.

## Regras financeiras

- Saldo real: inicial + entradas − saídas não canceladas, em todas as datas.
- Saldo bancário: inicial + entradas compensadas − saídas compensadas, em todas as datas.
- Saldo anterior: inicial + movimentos não cancelados anteriores à primeira data.
- Saldo final do período: anterior + entradas − saídas do período.
- Arquivar somente oculta o registro do filtro padrão; não altera nenhum saldo.
- Cancelados não participam dos totais, mesmo quando exibidos.
- Totais incluem todos os registros elegíveis, não somente a página visível.
- Centavos inteiros e somas em i128; nenhum cálculo financeiro usa ponto flutuante.
- Conta com lançamentos não permite modificar saldo inicial; use lançamento explícito de ajuste. Histórico usado não permite modificar sentido. A regra inclui registros cancelados.
- Cadastros inativos podem ser preservados ao corrigir um lançamento existente; novos vínculos exigem cadastros ativos.

## Gravação e isolamento

- Somente administradores alteram lançamentos. Consulta pode ver extrato e saldos.
- Identidade e ambiente vêm da sessão. RLS obrigatório e chaves estrangeiras compostas impedem referências entre clientes.
- O cliente envia UUID aleatório por inclusão, preservado enquanto o envio não for confirmado. Repetir a mesma chave e conteúdo retorna o registro existente. Usar a chave com conteúdo diferente retorna 409.
- A chave permanece no rascunho em memória da página, não sobrevive a recarregamento/fechamento. Em falha de conexão, consulte a lista antes de iniciar outro rascunho.
- Edições/cancelamentos exigem versão. Mudanças concorrentes com versão antiga retornam 409.
- Transações de escrita de lançamentos e cadastros são serializadas por ambiente com lock transacional PostgreSQL; ambientes distintos podem gravar em paralelo.
- Extrato lê conta e movimentos na mesma consulta SQL para evitar combinar saldos de instantes diferentes.
- Não há exclusão física pela API. Cancelamento preserva o registro, mas não equivale a um histórico completo de todas as versões editadas.

## API

- GET /api/entries?account_id=UUID&from=AAAA-MM-DD&to=AAAA-MM-DD&include_archived=true&include_cancelled=true&offset=0
- POST /api/entries: account_id, history_id, date, document, description, amount (texto), cleared, archived, request_id (UUID).
- GET /api/entries/{id}
- PUT /api/entries/{id}: mesmos campos financeiros, expected_version; sem request_id.
- POST /api/entries/{id}/cancel: expected_version.

Datas, filtros e campos são validados; tenant_id fornecido pelo cliente é rejeitado.

## Operação e limites

Build.ps1 compila API/WASM; Start-Dev.ps1 faz backup completo em .dev/backups e compara identidades e tabelas financeiras existentes antes/depois das migrações. Não imprime os dados privados. O backup local não substitui uma rotina de produção.

Test-Cycle2.ps1 mantém o nome para compatibilidade e executa a regressão dos ciclos 2, 3 e 4 em banco descartável. O cenário financeiro usa pool de quatro conexões, inclusive para reenvios e edições simultâneas.

Este ciclo não inclui importação do SQLite, PDF, conciliação com banco externo, transferências automáticas em pares, baixa em lote ou trilha completa de auditoria das versões. Linux/Docker continua pendente de execução. A consulta pagina a resposta, mas ainda lê os movimentos da conta no servidor para totalizar; grandes volumes precisam de agregação/paginação SQL e teste de carga antes de produção.
