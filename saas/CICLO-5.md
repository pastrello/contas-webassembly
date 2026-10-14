# Ciclo 5 — relatórios e consultas

Workspace Rust 0.5.0, branch codex/saas-cycle-5.

## Uso

Abra **Relatórios**, escolha **Resumo por conta** ou **Totais por histórico**, informe opcionalmente as datas e clique **Gerar relatório**. As duas datas são inclusivas; campos vazios abrangem todo o histórico.

O relatório considera todos os cadastros do ambiente, inclusive inativos. Arquivados participam dos totais; cancelados não participam. Administradores e usuários de consulta podem visualizar e exportar.

- **Resumo por conta:** saldo inicial, saldo anterior ao período, entradas, saídas, saldo final e saldo bancário até a data final. O bancário inclui os compensados anteriores ao período, além dos compensados dentro dele.
- **Totais por histórico:** entradas, saídas e resultado líquido dentro do período, somados entre todas as contas do ambiente. Não inclui saldo inicial.
- A última linha soma os valores das linhas do relatório. Cadastros sem movimento aparecem com zero (ou saldo inicial, no resumo de contas).

Os saldos de relatório respeitam a data final. Já os saldos real/bancário da aba Lançamentos continuam considerando todas as datas; a distinção aparece nos rótulos.

## CSV

**Baixar CSV atualizado deste período** gera um arquivo com os últimos filtros efetivamente consultados. Alterar campos sem clicar Gerar relatório não altera o link. O download consulta novamente o banco e pode refletir alterações posteriores à tela.

Formato UTF-8 com BOM, separador ponto e vírgula, valores decimais com vírgula, cabeçalhos e período dentro do arquivo. Aspas e quebras de linha são escapadas. Campos potencialmente interpretados como fórmulas recebem apóstrofo de proteção; isso inclui números negativos, que podem ser importados como texto na planilha.

O download exige sessão válida e respeita o ambiente da sessão. Não existe link público para os dados. Respostas têm no-store. Os arquivos baixados ficam sob controle do usuário.

## Consulta eficiente

O extrato agora calcula totais no PostgreSQL e busca no máximo 200 lançamentos por página, sem materializar todos os movimentos na memória da API. Totais e página são obtidos na mesma instrução SQL, mantendo consistência do instante consultado.

As somas SQL usam numeric; os totais são transferidos como texto e convertidos para inteiro de 128 bits. Valores de página mantêm duas casas decimais. Nenhum cálculo financeiro usa ponto flutuante.

As agregações ainda precisam percorrer os registros pertinentes; essa alteração não substitui medição de carga. Os relatórios agregados retornam todos os cadastros, sem paginação, apropriados ao protótipo atual.

## API

- GET /api/reports?kind=accounts&from=AAAA-MM-DD&to=AAAA-MM-DD
- GET /api/reports?kind=histories&from=AAAA-MM-DD&to=AAAA-MM-DD
- GET /api/reports/export com os mesmos parâmetros: CSV autenticado.

A resposta JSON inclui tipo, período, colunas e linhas formatadas para exibição, incluindo TOTAL. Não é uma API de cálculo a partir de valores de ponto flutuante.

## Preservação e pendências

Nenhuma migração de esquema foi necessária neste ciclo. Start-Dev continua criando backup e comparando identidades, contas, históricos e lançamentos existentes. O desktop C#/SQLite, instaladores e releases permanecem separados.

A conferência visual automatizada permanece indisponível. Linux/Docker, carga em produção, PDF, importação do SQLite e trilha completa de versões continuam pendentes.
