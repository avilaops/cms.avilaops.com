import { useEffect, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { createPortal } from 'react-dom';
import './editor.css';

// Contract editor-shell/1: Rust owns fields, permitted actions and submission.
// React owns navigation and interaction, without copying editorial rules.
const form = document.querySelector<HTMLFormElement>('form[data-editor-contract="1"]');
const root = document.getElementById('editor-workspace');

function fingerprint(form: HTMLFormElement): string {
  return JSON.stringify([...new FormData(form).entries()]);
}

function MediaSearch({ select }: { select: HTMLSelectElement }) {
  const [query, setQuery] = useState('');
  useEffect(() => {
    const term = query.toLocaleLowerCase('pt-BR');
    for (const option of select.options) {
      option.hidden = Boolean(term && option.value && !option.selected &&
        !option.text.toLocaleLowerCase('pt-BR').includes(term));
    }
    return () => { for (const option of select.options) option.hidden = false; };
  }, [query, select]);
  return <label className="editor-search">Buscar imagem
    <input type="search" value={query} onChange={e => setQuery(e.target.value)}
      aria-controls={select.id} placeholder="Digite parte da descrição" />
  </label>;
}

function Workspace({ form }: { form: HTMLFormElement }) {
  const sections = [...form.querySelectorAll<HTMLElement>('[data-editor-section]')];
  const [active, setActive] = useState(sections[0]?.id ?? '');
  const [dirty, setDirty] = useState(false);
  const [expanded, setExpanded] = useState(false);
  const [preview, setPreview] = useState(false);
  const [mobile, setMobile] = useState(false);
  const [mediaSlots] = useState(() => [...form.querySelectorAll<HTMLSelectElement>('[data-editor-media]')]
    .map(select => {
      const host = document.createElement('div');
      select.before(host);
      return { select, host };
    }));
  const [initial] = useState(() => fingerprint(form));
  const previewUrl = root?.dataset.preview;
  const previewSlot = document.getElementById('editor-preview-slot');

  useEffect(() => {
    document.body.classList.add('editor-ready');
    const update = () => setDirty(fingerprint(form) !== initial);
    const submitting = () => window.removeEventListener('beforeunload', leaving);
    const leaving = (event: BeforeUnloadEvent) => {
      if (fingerprint(form) !== initial) { event.preventDefault(); event.returnValue = ''; }
    };
    const shortcut = (event: KeyboardEvent) => {
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 's') {
        event.preventDefault();
        const save = form.querySelector<HTMLButtonElement>('button[value="salvar"]');
        if (save) form.requestSubmit(save);
      }
    };
    form.addEventListener('input', update);
    form.addEventListener('change', update);
    form.addEventListener('submit', submitting);
    window.addEventListener('beforeunload', leaving);
    window.addEventListener('keydown', shortcut);
    return () => {
      document.body.classList.remove('editor-ready');
      form.removeEventListener('input', update);
      form.removeEventListener('change', update);
      form.removeEventListener('submit', submitting);
      window.removeEventListener('beforeunload', leaving);
      window.removeEventListener('keydown', shortcut);
    };
  }, [form, initial]);

  useEffect(() => {
    for (const section of sections) section.hidden = !expanded && section.id !== active;
    return () => { for (const section of sections) section.hidden = false; };
  }, [active, expanded]);

  return <>
    <div className="editor-toolbar">
      <div><strong>Editor de conteúdo</strong>
        <p role="status">{dirty ? 'Alterações não salvas' : 'Nenhuma alteração pendente'}</p></div>
      <button type="submit" form={form.id} name="acao" value="salvar">Salvar rascunho</button>
      {previewUrl && <button type="button" className="secundario" onClick={() => setPreview(!preview)}>
        {preview ? 'Fechar prévia' : 'Ver prévia salva'}</button>}
    </div>
    <nav className="editor-outline" aria-label="Seções do documento">
      <h2>Documento</h2>
      {sections.map((section, i) => <button key={section.id} type="button"
        aria-current={active === section.id ? 'step' : undefined}
        onClick={() => { setActive(section.id); setExpanded(false); }}>
        <span>{String(i + 1).padStart(2, '0')}</span> {section.dataset.editorSection}
      </button>)}
      <button type="button" className="secundario" aria-pressed={expanded}
        onClick={() => setExpanded(!expanded)}>{expanded ? 'Uma seção por vez' : 'Mostrar todas'}</button>
    </nav>
    {mediaSlots.map(({ select, host }) => createPortal(<MediaSearch select={select} />, host, select.id))}
    {preview && previewUrl && previewSlot && createPortal(<section className="editor-preview" aria-label="Prévia salva">
      <div className="editor-preview-bar"><strong>Última versão salva</strong>
        <button type="button" aria-pressed={mobile} onClick={() => setMobile(!mobile)}>
          {mobile ? 'Ver desktop' : 'Ver celular'}</button>
        <a href={previewUrl} target="_blank" rel="noopener">Abrir em outra aba</a></div>
      {dirty && <p role="status">Salve o rascunho para incluir suas alterações na prévia.</p>}
      <iframe title="Prévia do rascunho salvo" src={previewUrl}
        className={mobile ? 'mobile' : ''} sandbox="allow-same-origin" />
    </section>, previewSlot)}
  </>;
}

if (root && form) createRoot(root).render(<Workspace form={form} />);
