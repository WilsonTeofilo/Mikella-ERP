-- Migration 0005: adiciona colunas `documento` e `endereco` na tabela `pessoas`.
--
-- `documento`: armazena o CPF ou CNPJ formatado ou apenas dígitos para pesquisa rápida e exibição no cupom fiscal.
-- `endereco`: endereço completo opcional para entregas e tele-entrega.
ALTER TABLE pessoas ADD COLUMN documento TEXT;
ALTER TABLE pessoas ADD COLUMN endereco TEXT;
