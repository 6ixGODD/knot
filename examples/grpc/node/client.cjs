// Direct Node.js gRPC client; no browser bridge or native KNOT library.
const grpc = require('@grpc/grpc-js');
const loader = require('@grpc/proto-loader');
const fs = require('node:fs');
const path = require('node:path');

async function main() {
  const [method, ...args] = process.argv.slice(2);
  if (!['Initial', 'Apply', 'Evaluate', 'Quote'].includes(method)) throw Error('method: Initial|Apply|Evaluate|Quote');
  const flags = {};
  while (args.length) {
    const key = args.shift();
    if (key === '--plaintext') flags[key] = true;
    else if (['--digest', '--endpoint', '--input', '--event', '--ca', '--cert', '--key'].includes(key) && args.length) flags[key] = args.shift();
    else throw Error('invalid argument: ' + key);
  }
  if (!flags['--digest']) throw Error('--digest is required');
  const proto = path.resolve(__dirname, '../../../contracts/grpc/knot/v1/configuration.proto');
  const definition = loader.loadSync(proto, {keepCase: true, longs: String, enums: String, bytes: Buffer, oneofs: true});
  const Service = grpc.loadPackageDefinition(definition).knot.v1.ConfigurationService;
  const read = key => {
    if (!flags[key]) throw Error(key + ' is required');
    return fs.readFileSync(flags[key]);
  };
  const tlsFlags = ['--ca', '--cert', '--key'];
  if (flags['--plaintext'] && tlsFlags.some(key => flags[key])) throw Error('plaintext cannot use TLS options');
  const credentials = flags['--plaintext'] ? grpc.credentials.createInsecure() : grpc.credentials.createSsl(read('--ca'), read('--key'), read('--cert'));
  const client = new Service(flags['--endpoint'] || '127.0.0.1:50051', credentials, {'grpc.max_receive_message_length': 4195328, 'grpc.enable_retries': 0});
  const request = {model_digest: flags['--digest']};
  if (method !== 'Initial') request[method === 'Quote' ? 'quote_json' : 'state_json'] = read('--input');
  if (method === 'Apply') request.event_json = read('--event');
  try {
    const response = await new Promise((resolve, reject) => client[method](request, {deadline: Date.now() + 10000}, (error, reply) => error ? reject(error) : resolve(reply)));
    const success = method === 'Quote' ? 'quote_json' : 'result_json';
    if (![success, 'error_json'].includes(response.outcome)) throw Error('missing or unknown response outcome');
    const value = JSON.parse(new TextDecoder('utf-8', {fatal: true}).decode(response[response.outcome]));
    if (!value || value.schemaVersion !== '1.0') throw Error('invalid response envelope');
    if (response.outcome === 'error_json') { console.error(JSON.stringify(value)); process.exitCode = 2; }
    else console.log(JSON.stringify(value));
  } finally { client.close(); }
}
main().catch(error => { console.error(error.message); process.exitCode = 1; });
