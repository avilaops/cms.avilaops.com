const root=document.querySelector('[data-cardapio-dados]');
const $=selector=>root.querySelector(selector);
const esc=escapeHtml;
const restaurant=root.dataset.name,phone=root.dataset.whatsapp;
const storageKey='cms-cardapio-'+root.dataset.scope+'-v1';
const bag='<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M5 7h14l1 14H4L5 7Zm4 0V5a3 3 0 0 1 6 0v2"/></svg>';
const wa='<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M20 11.5a8.5 8.5 0 0 1-12.5 7.4L3 20l1.2-4.3A8.5 8.5 0 1 1 20 11.5Z"/><path d="M8 8c0 4 4 7 7 7l1-2-2-1-1 1c-2-1-3-2-3-3l1-1-1-2-2 1Z"/></svg>';
let categories=[],products=new Map(),borders=new Map(),cart=[],active=root.dataset.active,ingredient='',query='',sort='original',selected=null,quantity=1,toastTimer,name='',delivery='';
const labels={};
const filters=[['','Todos'],['calabresa','Calabresa'],['frango','Frango'],['queijo','Queijos'],['lombo','Lombo'],['atum','Atum'],['carne','Carnes']];
function announce(message){$('#toast').textContent=message;$('#toast').classList.add('visible');clearTimeout(toastTimer);toastTimer=setTimeout(()=>$('#toast').classList.remove('visible'),2600);}
function storeCart(){try{localStorage.setItem(storageKey,JSON.stringify(cart));}catch{}}
function loadCart(){try{cart=cleanCart(JSON.parse(localStorage.getItem(storageKey)||'[]'),products,borders);}catch{cart=[];}}
function renderTabs(){
  $('#category-tabs').innerHTML=categories.map(c=>`<button role="tab" type="button" id="tab-${c.id}" aria-controls="menu-panel" aria-selected="${c.id===active}" tabindex="${c.id===active?'0':'-1'}" data-category="${c.id}">${esc(labels[c.id])} <small>${c.items.length}</small></button>`).join('');
  $('#menu-panel').setAttribute('aria-labelledby','tab-'+active);
  const current=categories.find(c=>c.id===active);
  $('#ingredient-filters').innerHTML=current.kind==='pizza'?filters.filter(([value])=>!value||current.items.some(p=>normalize(p.name+' '+p.ingredients).includes(value))).map(([value,label])=>`<button type="button" class="${ingredient===value?'active':''}" aria-pressed="${ingredient===value}" data-ingredient="${value}">${label}</button>`).join(''):'';
}
function matchIngredient(p){const text=normalize(p.ingredients);switch(ingredient){case 'queijo':return /queijo|mussarela|provolone|gorgonzola/.test(normalize(p.name));case 'carne':return /carne seca|cupim|costela/.test(text);default:return text.includes(ingredient);}}
function renderProducts(){
  let rows=categories.find(c=>c.id===active).items.filter(p=>(!query||normalize(p.name+' '+p.ingredients).includes(query))&&(!ingredient||matchIngredient(p)));
  if(sort==='price') rows=[...rows].sort((a,b)=>a.price-b.price);
  if(sort==='name') rows=[...rows].sort((a,b)=>a.name.localeCompare(b.name,'pt-BR'));
  $('#result-count').textContent=`${rows.length} ${rows.length===1?'opção encontrada':'opções para escolher'}`;
  $('#products').innerHTML=rows.length?rows.map(p=>`<button class="product" data-product="${esc(p.id)}" aria-label="Escolher ${esc(p.name)}, ${esc(money(p.price))}"><span class="product-top"><span class="product-index">${String(categories.find(c=>c.id===active).items.indexOf(p)+1).padStart(2,'0')} / ${esc(labels[active].toUpperCase())}</span></span><h3>${esc(p.name)}</h3><p class="product-description">${esc(p.ingredients||(categories.find(c=>c.id===active).kind==='borda'?'Adicional para a sua pizza.':'Para acompanhar o seu pedido.'))}</p><span class="product-bottom"><strong class="product-price">${money(p.price)}</strong><span class="product-plus" aria-hidden="true">+</span></span></button>`).join(''):'<div class="empty-results">Nenhuma opção encontrada.<br>Tente outro sabor ou ingrediente.<button data-reset>Limpar filtros</button></div>';
  $('#menu-note').textContent=(categories.find(c=>c.id===active).kind==='borda'?'As bordas são adicionais à pizza. ':'')+'Tamanho, disponibilidade e entrega são confirmados pela equipe no WhatsApp.';
}
function chooseCategory(id){active=id;ingredient='';query='';$('#search').value='';renderTabs();renderProducts();}
function openProduct(id){
  selected=products.get(id);quantity=1;
  const pizza=selected.kind==='pizza';
  $('#product-detail').innerHTML=`<p class="detail-eyebrow">${esc(categories.find(c=>c.id===selected.category).name)}</p><h2 id="product-title">${esc(selected.name)}</h2><p class="detail-description">${esc(selected.ingredients||(selected.kind==='borda'?'Adicional para a sua pizza.':'Para acompanhar o seu pedido.'))}</p><p class="detail-price">${money(selected.price)}</p>${pizza&&borders.size?`<label class="detail-label">Quer uma borda recheada?<select id="border-choice"><option value="">Sem borda recheada adicional</option>${[...borders.values()].map(b=>`<option value="${b.id}">${esc(b.name)} (+ ${money(b.price)})</option>`).join('')}</select></label>`:''}<label class="detail-label">Alguma observação? <span style="font-weight:400">(opcional)</span><textarea id="item-note" placeholder="Conte para a equipe o que você precisa." maxlength="240"></textarea></label><p class="detail-disclosure">A equipe confirma ${pizza?'tamanho, ':''}disponibilidade e valor final no WhatsApp.</p><div class="detail-actions"><div class="quantity-control"><button type="button" id="detail-minus" aria-label="Diminuir quantidade" disabled>−</button><output id="detail-quantity" aria-live="polite">1</output><button type="button" id="detail-plus" aria-label="Aumentar quantidade">+</button></div><button class="button dark-button" id="add-to-order">Adicionar · <span id="detail-total" style="font-size:inherit">${money(selected.price)}</span></button></div>`;
  $('#product-dialog').showModal();
}
function detailTotal(){const border=borders.get($('#border-choice')?.value);$('#detail-quantity').textContent=quantity;$('#detail-minus').disabled=quantity<=1;$('#detail-plus').disabled=quantity>=99;$('#detail-total').textContent=money((selected.price+(border?.price||0))*quantity);}
function checkoutLink(){return 'https://wa.me/'+phone+'?text='+encodeURIComponent(orderMessage(cart,products,borders,name,delivery,restaurant));}
function renderOrder(){
  const total=cartTotals(cart,products,borders);
  const content=`<div class="order-title"><h3>Seu pedido</h3><span>${total.count} ${total.count===1?'item':'itens'}</span></div>${cart.length?`<ul class="order-items">${cart.map((r,i)=>{const p=products.get(r.id),b=borders.get(r.borderId);return `<li class="order-item"><p class="order-item-name">${esc(p.name)}</p>${b?'<p class="order-item-note">+ '+esc(b.name)+'</p>':''}${r.note?'<p class="order-item-note">'+esc(r.note)+'</p>':''}<div class="order-item-footer"><div class="quantity-control"><button data-quantity="${i}" data-delta="-1" aria-label="${r.quantity===1?'Remover':'Diminuir quantidade de'} ${esc(p.name)}">−</button><span>${r.quantity}</span><button data-quantity="${i}" data-delta="1" aria-label="Aumentar quantidade de ${esc(p.name)}" ${r.quantity>=99?'disabled':''}>+</button></div><strong>${money((p.price+(b?.price||0))*r.quantity)}</strong></div></li>`;}).join('')}</ul><div class="order-fields"><label>Seu nome (opcional)<input data-field="name" autocomplete="given-name" maxlength="80" value="${esc(name)}" placeholder="Como podemos chamar você?"></label><label>Endereço para consultar entrega (opcional)<textarea data-field="delivery" autocomplete="street-address" maxlength="300" placeholder="Rua, número e bairro">${esc(delivery)}</textarea></label></div>`:`<div class="empty-order"><div class="empty-symbol">${bag}</div><h4>Uma boa noite começa aqui.</h4><p>Escolha seus sabores favoritos.<br>A gente organiza tudo para você.</p></div>`}<div class="order-totals"><span>Subtotal dos itens</span><strong>${money(total.cents)}</strong></div><p class="order-note">Entrega não incluída. Disponibilidade, tamanho e valor final confirmados no atendimento.</p>${cart.length?`<a class="checkout" href="${esc(checkoutLink())}" data-checkout target="_blank" rel="noopener">${wa} Combinar pelo WhatsApp <span aria-hidden="true">↗</span></a>`:`<button class="checkout" disabled>${wa} Escolha seus itens</button>`}<p class="order-foot">Você revisa a mensagem antes de enviar.</p>`;
  $('#desktop-order').innerHTML=content;$('#mobile-order').innerHTML=content;
  root.querySelectorAll('[data-count]').forEach(e=>e.textContent=total.count);
  $('#mobile-total').textContent=money(total.cents);
}
function addToOrder(){
  const row={id:selected.id,quantity,borderId:$('#border-choice')?.value||'',note:$('#item-note').value.trim()};
  const existing=cart.find(r=>r.id===row.id&&r.borderId===row.borderId&&r.note===row.note);
  if(existing && existing.quantity+quantity>99){announce('Limite de 99 unidades por item.');return;}
  if(!existing&&cart.length>=100){announce('Limite de itens atingido. Combine seu pedido com a equipe.');return;}
  if(existing)existing.quantity+=quantity;else cart.push(row);
  storeCart();renderOrder();$('#product-dialog').close();announce(selected.name+' adicionado ao pedido.');
}
root.addEventListener('click',event=>{
  const target=event.target.closest('button,a');if(!target)return;
  if(target.hasAttribute('data-product'))openProduct(target.dataset.product);
  if(target.hasAttribute('data-category'))chooseCategory(target.dataset.category);
  if(target.hasAttribute('data-ingredient')){ingredient=target.dataset.ingredient;renderTabs();renderProducts();}
  if(target.hasAttribute('data-reset')){ingredient='';query='';$('#search').value='';renderTabs();renderProducts();}
  if(target.id==='detail-minus'){quantity=Math.max(1,quantity-1);detailTotal();}
  if(target.id==='detail-plus'){quantity=Math.min(99,quantity+1);detailTotal();}
  if(target.id==='add-to-order')addToOrder();
  if(target.hasAttribute('data-quantity')){const i=Number(target.dataset.quantity),r=cart[i];r.quantity=Math.min(99,r.quantity+Number(target.dataset.delta));if(r.quantity<=0)cart.splice(i,1);storeCart();renderOrder();}
  if(target.hasAttribute('data-cart-open')){if(!products.size)return;$('#cart-dialog').showModal();}
});
$('#category-tabs').addEventListener('keydown',event=>{const tabs=[...$('#category-tabs').children];let index=tabs.indexOf(event.target.closest('[role="tab"]'));if(index<0)return;if(event.key==='ArrowRight')index=(index+1)%tabs.length;else if(event.key==='ArrowLeft')index=(index-1+tabs.length)%tabs.length;else if(event.key==='Home')index=0;else if(event.key==='End')index=tabs.length-1;else return;event.preventDefault();chooseCategory(tabs[index].dataset.category);$('#tab-'+active).focus();});
$('#search').addEventListener('input',()=>{query=normalize($('#search').value.trim());renderProducts();});
$('#sort').addEventListener('change',()=>{sort=$('#sort').value;renderProducts();});
root.addEventListener('change',event=>{if(event.target.id==='border-choice')detailTotal();});
root.addEventListener('input',event=>{const field=event.target.dataset.field;if(!['name','delivery'].includes(field))return;if(field==='name')name=event.target.value;else delivery=event.target.value;root.querySelectorAll(`[data-field="${field}"]`).forEach(el=>{if(el!==event.target)el.value=event.target.value;});root.querySelectorAll('[data-checkout]').forEach(el=>el.href=checkoutLink());});
for(const dialog of root.querySelectorAll('dialog'))dialog.addEventListener('click',event=>{if(event.target===dialog){const r=dialog.getBoundingClientRect();if(event.clientX<r.left||event.clientX>r.right||event.clientY<r.top||event.clientY>r.bottom)dialog.close();}});

try{
  categories=JSON.parse(root.dataset.menu);
  for(const c of categories)labels[c.id]=c.name;
  for(const c of categories)for(const p of c.items)products.set(p.id,{...p,category:c.id,kind:c.kind});
  borders=new Map(categories.filter(c=>c.kind==='borda').flatMap(c=>c.items).map(p=>[p.id,p]));loadCart();renderTabs();renderProducts();renderOrder();
  $('[data-cardapio-app]').hidden=false;
  $('[data-cardapio-alternativa]').hidden=true;
}catch{console.error('Não foi possível abrir a seleção de itens; o cardápio permanece disponível.');}
