import test from 'node:test';
import assert from 'node:assert/strict';
import {cleanCart,cartTotals,orderMessage,escapeHtml} from './pedido.mjs';

const products=new Map([
  ['pizza',{name:'Pizza de queijo',kind:'pizza',price:3590}],
  ['bebida',{name:'Água',kind:'produto',price:550}],
]);
const borders=new Map([['borda',{name:'Borda de queijo',price:1200}]]);

test('soma em centavos inclui a borda por unidade e mantém a mensagem para revisão',()=>{
  const cart=cleanCart([{id:'pizza',quantity:2,borderId:'borda',note:'Sem cebola'},{id:'bebida',quantity:1}],products,borders);
  assert.deepEqual(cartTotals(cart,products,borders),{count:3,cents:10130});
  const message=orderMessage(cart,products,borders,'Ana','Rua Exemplo, 10','Pizzaria Exemplo');
  assert.match(message,/Olá, Pizzaria Exemplo!/);
  assert.match(message,/Borda de queijo/);
  assert.match(message,/Sem cebola/);
  assert.match(message,/101,30/);
  assert.match(message,/entrega não incluída/);
});

test('carrinho persistido não aceita produto removido, borda em bebida ou quantidade inválida',()=>{
  const raw=[null,{id:'removido',quantity:1},{id:'bebida',quantity:1,borderId:'borda'},{id:'pizza',quantity:0},{id:'pizza',quantity:1.5},{id:'pizza',quantity:100},{id:'pizza',quantity:1,borderId:'removida'},{id:'pizza',quantity:1,note:'a'.repeat(300)}];
  const cart=cleanCart(raw,products,borders);
  assert.equal(cart.length,1);
  assert.equal(cart[0].note.length,240);
  assert.deepEqual(cleanCart({quantity:1},products,borders),[]);
});

test('revalida o preço pelo catálogo atual em vez de confiar no valor do armazenamento',()=>{
  const cart=cleanCart([{id:'pizza',quantity:1,price:1}],products,borders);
  assert.equal(cartTotals(cart,products,borders).cents,3590);
  assert.equal(escapeHtml('<img src=x onerror="alert(1)">'),'&lt;img src=x onerror=&quot;alert(1)&quot;&gt;');
});
