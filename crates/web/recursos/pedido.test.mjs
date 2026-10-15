import test from 'node:test';
import assert from 'node:assert/strict';
import {cleanCart, cartTotals, orderMessage} from './pedido.mjs';

const products = new Map([
  ['pizza', {name:'Pizza de exemplo', price:3590, kind:'pizza'}],
  ['bebida', {name:'Bebida de exemplo', price:900, kind:'produto'}],
  ['borda', {name:'Borda de exemplo', price:1200, kind:'borda'}]
]);
const borders = new Map([['borda', products.get('borda')]]);

test('quantidade multiplica a pizza e a sua borda, sem arredondar', () => {
  const cart = [{id:'pizza', quantity:2, borderId:'borda', note:'Sem cebola'}, {id:'bebida', quantity:1}];
  assert.deepEqual(cartTotals(cart,products,borders), {count:3,cents:10480});
  const text = orderMessage(cart,products,borders,'Ana','Rua do Exemplo, 12','Restaurante Exemplo');
  assert.match(text, /2 × Pizza de exemplo \+ Borda de exemplo/);
  assert.match(text, /104,80/);
  assert.match(text, /entrega não incluída/);
  assert.match(text, /Sem cebola/);
  assert.match(text, /Rua do Exemplo, 12/);
});

test('seleção salva é revalidada contra o catálogo atual', () => {
  assert.deepEqual(cleanCart([
    {id:'removido',quantity:1},
    {id:'pizza',quantity:-1},
    {id:'pizza',quantity:1.5},
    {id:'pizza',quantity:100},
    {id:'bebida',quantity:1,borderId:'borda'},
    {id:'pizza',quantity:1,borderId:'removida'},
    {id:'pizza',quantity:1,borderId:'borda',note:'x'.repeat(500)}
  ],products,borders), [{id:'pizza',quantity:1,borderId:'borda',note:'x'.repeat(240)}]);
  assert.deepEqual(cleanCart({},products,borders), []);
  const changed = new Map(products);
  changed.set('pizza',{...products.get('pizza'),price:4200});
  const saved = cleanCart([{id:'pizza',quantity:2,price:1}],changed,borders);
  assert.equal(cartTotals(saved,changed,borders).cents,8400);
});
