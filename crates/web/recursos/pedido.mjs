export const money = cents => (cents / 100).toLocaleString('pt-BR', {style:'currency',currency:'BRL'});
export const normalize = text => text.normalize('NFD').replace(/[\u0300-\u036f]/g,'').toLowerCase();
export const escapeHtml = value => String(value).replace(/[&<>"']/g, char => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[char]));
export function cleanCart(raw, products, borders) {
  if (!Array.isArray(raw)) return [];
  return raw.slice(0,100).filter(row => row && typeof row==='object' && products.has(row.id) && Number.isInteger(row.quantity) && row.quantity>0 && row.quantity<=99 && (!row.borderId || (products.get(row.id).kind==='pizza' && borders.has(row.borderId)))).map(row => ({id:row.id,quantity:row.quantity,borderId:row.borderId||'',note:typeof row.note==='string'?row.note.slice(0,240):''}));
}
export function cartTotals(cart, products, borders) {
  return cart.reduce((sum,row) => {
    const item=products.get(row.id), border=borders.get(row.borderId);
    return {count:sum.count+row.quantity,cents:sum.cents+(item.price+(border?.price||0))*row.quantity};
  },{count:0,cents:0});
}
export function orderMessage(cart, products, borders, name='', delivery='', restaurant='Restaurante') {
  const rows=cart.map(row => {
    const item=products.get(row.id),border=borders.get(row.borderId);
    return `${row.quantity} × ${item.name}${border?' + '+border.name:''} — ${money((item.price+(border?.price||0))*row.quantity)}${row.note?'\n  Observação: '+row.note:''}`;
  });
  const total=cartTotals(cart,products,borders);
  return [`Olá, ${restaurant}! Gostaria de combinar este pedido:`,name.trim()?'Nome: '+name.trim().slice(0,80):'', '',...rows,'',`Subtotal dos itens: ${money(total.cents)} (entrega não incluída).`,delivery.trim()?'Endereço/referência: '+delivery.trim().slice(0,300):'','', 'Podem confirmar o tamanho das pizzas, disponibilidade, taxa de entrega, valor final e formas de pagamento?'].filter((line,i,all)=>line!==''||all[i-1]!=='').join('\n');
}
