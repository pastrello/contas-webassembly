namespace Caixa.WinForms.Utilities;

/// <summary>
/// Guarda um retrato textual dos campos editáveis e alerta ao fechar uma tela
/// quando o conteúdo atual diverge do último estado carregado/salvo.
/// </summary>
internal sealed class UnsavedChangesGuard
{
    private string? _acceptedState;

    public void Accept(string state) => _acceptedState = state;

    public bool IsDirty(string state)
        => _acceptedState is not null && !string.Equals(_acceptedState, state, StringComparison.Ordinal);

    public bool ConfirmClose(IWin32Window owner, string currentState)
    {
        if (!IsDirty(currentState)) return true;

        return MessageBox.Show(owner,
            "Existem alterações não salvas nesta tela.\r\n\r\nDeseja fechar e descartar essas alterações?",
            "Alterações não salvas",
            MessageBoxButtons.YesNo,
            MessageBoxIcon.Warning,
            MessageBoxDefaultButton.Button2) == DialogResult.Yes;
    }

    public bool ConfirmDiscard(IWin32Window owner, string currentState)
    {
        if (!IsDirty(currentState)) return true;
        return MessageBox.Show(owner,
            "Existem alterações não salvas.\r\n\r\nDeseja descartá-las e iniciar um novo lançamento?",
            "Alterações não salvas", MessageBoxButtons.YesNo, MessageBoxIcon.Warning,
            MessageBoxDefaultButton.Button2) == DialogResult.Yes;
    }
}
