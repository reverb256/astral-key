'use strict';

/**
 * Keychain Module — BIP39 seed phrases, key derivation, encryption at rest,
 * and social recovery (Shamir's Secret Sharing over GF(256)).
 *
 * Key format (matching identity.js):
 *   - Public key:  32 bytes, Base64URL encoded (44 chars, no padding)
 *   - Private key: 64 bytes (seed || public key), Base64URL encoded
 *   - pubkeyHex:   hex-encoded public key
 *
 * Dependencies:
 *   - tweetnacl (Ed25519)
 *   - Node.js built-in crypto (PBKDF2, AES-256-GCM, SHA256)
 */

const nacl = require('tweetnacl');
const crypto = require('crypto');

// ─── BIP39 English Wordlist ────────────────────────────────────────────────
// 2048 words, alphabetical, as defined by BIP-0039.
// https://github.com/bitcoin/bips/blob/master/bip-0039/english.txt

const BIP39_WORDS = [
  'abandon','ability','able','about','above','absent','absorb','abstract',
  'absurd','abuse','access','accident','account','accuse','achieve','acid',
  'acoustic','acquire','across','act','action','actor','actress','actual',
  'adapt','add','addict','address','adjust','admit','adult','advance',
  'advice','aerobic','affair','afford','afraid','again','age','agent',
  'agree','ahead','aim','air','airport','aisle','alarm','album','alcohol',
  'alert','alien','all','alley','allow','almost','alone','alpha','already',
  'also','alter','always','amateur','amazing','among','amount','amused',
  'analyst','anchor','ancient','anger','angle','angry','animal','ankle',
  'announce','annual','another','answer','antenna','antique','anxiety','any',
  'apart','apology','appear','apple','approve','april','arch','arctic',
  'area','arena','argue','arm','armed','armor','army','around','arrange',
  'arrest','arrive','arrow','art','artefact','artist','artwork','ask',
  'aspect','assault','asset','assist','assume','asthma','athlete','atom',
  'attack','attend','attitude','attract','auction','audit','august','aunt',
  'author','auto','autumn','average','avocado','avoid','awake','aware',
  'away','awesome','awful','awkward','axis','baby','bachelor','bacon',
  'badge','bag','balance','balcony','ball','bamboo','banana','banner','bar',
  'barely','bargain','barrel','base','basic','basket','battle','beach',
  'bean','beauty','because','become','beef','before','begin','behave',
  'behind','believe','below','belt','bench','benefit','best','betray',
  'better','between','beyond','bicycle','bid','bike','bind','biology','bird',
  'birth','bitter','black','blade','blame','blanket','blast','bleak','bless',
  'blind','blood','blossom','blouse','blue','blur','blush','board','boat',
  'body','boil','bomb','bone','bonus','book','boost','border','boring',
  'borrow','boss','bottom','bounce','box','boy','bracket','brain','brand',
  'brass','brave','bread','breeze','brick','bridge','brief','bright','bring',
  'brisk','broccoli','broken','bronze','broom','brother','brown','brush',
  'bubble','buddy','budget','buffalo','build','bulb','bulk','bullet',
  'bundle','bunker','burden','burger','burst','bus','business','busy',
  'butter','buyer','buzz','cabbage','cabin','cable','cactus','cage','cake',
  'call','calm','camera','camp','can','canal','cancel','candy','cannon',
  'canoe','canvas','canyon','capable','capital','captain','car','carbon',
  'card','cargo','carpet','carry','cart','case','cash','casino','castle',
  'casual','cat','catalog','catch','category','cattle','caught','cause',
  'caution','cave','ceiling','celery','cement','census','century','cereal',
  'certain','chair','chalk','champion','change','chaos','chapter','charge',
  'chase','chat','cheap','check','cheese','chef','cherry','chest','chicken',
  'chief','child','chimney','choice','choose','chronic','chuckle','chunk',
  'churn','cigar','cinnamon','circle','citizen','city','civil','claim',
  'clap','clarify','claw','clay','clean','clerk','clever','click','client',
  'cliff','climb','clinic','clip','clock','clog','close','cloth','cloud',
  'clown','club','clump','cluster','clutch','coach','coast','coconut','code',
  'coffee','coil','coin','collect','color','column','combine','come',
  'comfort','comic','common','company','concert','conduct','confirm',
  'congress','connect','consider','control','convince','cook','cool','copper',
  'copy','coral','core','corn','correct','cost','cotton','couch','country',
  'couple','course','cousin','cover','coyote','crack','cradle','craft',
  'cram','crane','crash','crater','crawl','crazy','cream','credit','creek',
  'crew','cricket','crime','crisp','critic','crop','cross','crouch','crowd',
  'crucial','cruel','cruise','crumble','crunch','crush','cry','crystal',
  'cube','culture','cup','cupboard','curious','current','curtain','curve',
  'cushion','custom','cute','cycle','dad','damage','damp','dance','danger',
  'daring','dash','daughter','dawn','day','deal','debate','debris','decade',
  'december','decide','decline','decorate','decrease','deer','defense',
  'define','defy','degree','delay','deliver','demand','demise','denial',
  'dentist','deny','depart','depend','deposit','depth','deputy','derive',
  'describe','desert','design','desk','despair','destroy','detail','detect',
  'develop','device','devote','diagram','dial','diamond','diary','dice',
  'diesel','diet','differ','digital','dignity','dilemma','dinner','dinosaur',
  'direct','dirt','disagree','discover','disease','dish','dismiss','disorder',
  'display','distance','divert','divide','divorce','dizzy','doctor',
  'document','dog','doll','dolphin','domain','donate','donkey','donor','door',
  'dose','double','dove','draft','dragon','drama','drastic','draw','dream',
  'dress','drift','drill','drink','drip','drive','drop','drum','dry','duck',
  'dumb','dune','during','dust','dutch','duty','dwarf','dynamic','eager',
  'eagle','early','earn','earth','easily','east','easy','echo','ecology',
  'economy','edge','edit','educate','effort','egg','eight','either','elbow',
  'elder','electric','elegant','element','elephant','elevator','elite','else',
  'embark','embody','embrace','emerge','emotion','employ','empower','empty',
  'enable','enact','end','endless','endorse','enemy','energy','enforce',
  'engage','engine','enhance','enjoy','enlist','enough','enrich','enroll',
  'ensure','enter','entire','entry','envelope','episode','equal','equip',
  'era','erase','erode','erosion','error','erupt','escape','essay','essence',
  'estate','eternal','ethics','evidence','evil','evoke','evolve','exact',
  'example','excess','exchange','excite','exclude','excuse','execute',
  'exercise','exhaust','exhibit','exile','exist','exit','exotic','expand',
  'expect','expire','explain','expose','express','extend','extra','eye',
  'eyebrow','fabric','face','faculty','fade','faint','faith','fall','false',
  'fame','family','famous','fan','fancy','fantasy','farm','fashion','fat',
  'fatal','father','fatigue','fault','favorite','feature','february',
  'federal','fee','feed','feel','female','fence','festival','fetch','fever',
  'few','fiber','fiction','field','figure','file','film','filter','final',
  'find','fine','finger','finish','fire','firm','first','fiscal','fish',
  'fit','fitness','fix','flag','flame','flash','flat','flavor','flee',
  'flight','flip','float','flock','floor','flower','fluid','flush','fly',
  'foam','focus','fog','foil','fold','follow','food','foot','force','forest',
  'forget','fork','fortune','forum','forward','fossil','foster','found','fox',
  'fragile','frame','frequent','fresh','friend','fringe','frog','front',
  'frost','frown','frozen','fruit','fuel','fun','funny','furnace','fury',
  'future','gadget','gain','galaxy','gallery','game','gap','garage','garbage',
  'garden','garlic','garment','gas','gasp','gate','gather','gauge','gaze',
  'general','genius','genre','gentle','genuine','gesture','ghost','giant',
  'gift','giggle','ginger','giraffe','girl','give','glad','glance','glare',
  'glass','glide','glimpse','globe','gloom','glory','glove','glow','glue',
  'goat','goddess','gold','good','goose','gorilla','gospel','gossip','govern',
  'gown','grab','grace','grain','grant','grape','grass','gravity','great',
  'green','grid','grief','grit','grocery','group','grow','grunt','guard',
  'guess','guide','guilt','guitar','gun','gym','habit','hair','half','hammer',
  'hamster','hand','happy','harbor','hard','harsh','harvest','hat','have',
  'hawk','hazard','head','health','heart','heavy','hedgehog','height','hello',
  'helmet','help','hen','hero','hidden','high','hill','hint','hip','hire',
  'history','hobby','hockey','hold','hole','holiday','hollow','home','honey',
  'hood','hope','horn','horror','horse','hospital','host','hotel','hour',
  'hover','hub','huge','human','humble','humor','hundred','hungry','hunt',
  'hurdle','hurry','hurt','husband','hybrid','ice','icon','idea','identify',
  'idle','ignore','ill','illegal','illness','image','imitate','immense',
  'immune','impact','impose','improve','impulse','inch','include','income',
  'increase','index','indicate','indoor','industry','infant','inflict',
  'inform','inhale','inherit','initial','inject','injury','inmate','inner',
  'innocent','input','inquiry','insane','insect','inside','inspire','install',
  'intact','interest','into','invest','invite','involve','iron','island',
  'isolate','issue','item','ivory','jacket','jaguar','jar','jazz','jealous',
  'jeans','jelly','jewel','job','join','joke','journey','joy','judge','juice',
  'jump','jungle','junior','junk','just','kangaroo','keen','keep','ketchup',
  'key','kick','kid','kidney','kind','kingdom','kiss','kit','kitchen','kite',
  'kitten','kiwi','knee','knife','knock','know','lab','label','labor',
  'ladder','lady','lake','lamp','language','laptop','large','later','latin',
  'laugh','laundry','lava','law','lawn','lawsuit','layer','lazy','leader',
  'leaf','learn','leave','lecture','left','leg','legal','legend','leisure',
  'lemon','lend','length','lens','leopard','lesson','letter','level','liar',
  'liberty','library','license','life','lift','light','like','limb','limit',
  'link','lion','liquid','list','little','live','lizard','load','loan',
  'lobster','local','lock','logic','lonely','long','loop','lottery','loud',
  'lounge','love','loyal','lucky','luggage','lumber','lunar','lunch','luxury',
  'lyrics','machine','mad','magic','magnet','maid','mail','main','major',
  'make','mammal','man','manage','mandate','mango','mansion','manual','maple',
  'marble','march','margin','marine','market','marriage','mask','mass',
  'master','match','material','math','matrix','matter','maximum','maze',
  'meadow','mean','measure','meat','mechanic','medal','media','melody','melt',
  'member','memory','mention','menu','mercy','merge','merit','merry','mesh',
  'message','metal','method','middle','midnight','milk','million','mimic',
  'mind','minimum','minor','minute','miracle','mirror','misery','miss',
  'mistake','mix','mixed','mixture','mobile','model','modify','mom','moment',
  'monitor','monkey','monster','month','moon','moral','more','morning',
  'mosquito','mother','motion','motor','mountain','mouse','move','movie',
  'much','muffin','mule','multiply','muscle','museum','mushroom','music',
  'must','mutual','myself','mystery','myth','naive','name','napkin','narrow',
  'nasty','nation','nature','near','neck','need','negative','neglect',
  'neither','nephew','nerve','nest','net','network','neutral','never','news',
  'next','nice','night','noble','noise','nominee','noodle','normal','north',
  'nose','notable','note','nothing','notice','novel','now','nuclear','number',
  'nurse','nut','oak','obey','object','oblige','obscure','observe','obtain',
  'obvious','occur','ocean','october','odor','off','offer','office','often',
  'oil','okay','old','olive','olympic','omit','once','one','onion','online',
  'only','open','opera','opinion','oppose','option','orange','orbit','orchard',
  'order','ordinary','organ','orient','original','orphan','ostrich','other',
  'outdoor','outer','output','outside','oval','oven','over','own','owner',
  'oxygen','oyster','ozone','pact','paddle','page','pair','palace','palm',
  'panda','panel','panic','panther','paper','parade','parent','park','parrot',
  'party','pass','patch','path','patient','patrol','pattern','pause','pave',
  'payment','peace','peanut','pear','peasant','pelican','pen','penalty',
  'pencil','people','pepper','perfect','permit','person','pet','phone','photo',
  'phrase','physical','piano','picnic','picture','piece','pig','pigeon',
  'pill','pilot','pink','pioneer','pipe','pistol','pitch','pizza','place',
  'planet','plastic','plate','play','please','pledge','pluck','plug','plunge',
  'poem','poet','point','polar','pole','police','pond','pony','pool','popular',
  'portion','position','possible','post','potato','pottery','poverty','powder',
  'power','practice','praise','predict','prefer','prepare','present','pretty',
  'prevent','price','pride','primary','print','priority','prison','private',
  'prize','problem','process','produce','profit','program','project','promote',
  'proof','property','prosper','protect','proud','provide','public','pudding',
  'pull','pulp','pulse','pumpkin','punch','pupil','puppy','purchase','purity',
  'purpose','purse','push','put','puzzle','pyramid','quality','quantum',
  'quarter','question','quick','quit','quiz','quote','rabbit','raccoon','race',
  'rack','radar','radio','rail','rain','raise','rally','ramp','ranch','random',
  'range','rapid','rare','rate','rather','raven','raw','razor','ready','real',
  'reason','rebel','rebuild','recall','receive','recipe','record','recycle',
  'reduce','reflect','reform','refuse','region','regret','regular','reject',
  'relax','release','relief','rely','remain','remember','remind','remove',
  'render','renew','rent','reopen','repair','repeat','replace','report',
  'require','rescue','resemble','resist','resource','response','result',
  'retire','retreat','return','reunion','reveal','review','reward','rhythm',
  'rib','ribbon','rice','rich','ride','ridge','rifle','right','rigid','ring',
  'riot','ripple','risk','ritual','rival','river','road','roast','robot',
  'robust','rocket','romance','roof','rookie','room','rose','rotate','rough',
  'round','route','royal','rubber','rude','rug','rule','run','runway','rural',
  'sad','saddle','sadness','safe','sail','salad','salmon','salon','salt',
  'salute','same','sample','sand','satisfy','satoshi','sauce','sausage',
  'save','say','scale','scan','scare','scatter','scene','scheme','school',
  'science','scissors','scorpion','scout','scrap','screen','script','scrub',
  'sea','search','season','seat','second','secret','section','security','seed',
  'seek','segment','select','sell','seminar','senior','sense','sentence',
  'series','service','session','settle','setup','seven','shadow','shaft',
  'shallow','share','shed','shell','sheriff','shield','shift','shine','ship',
  'shiver','shock','shoe','shoot','shop','short','shoulder','shove','shrimp',
  'shrug','shuffle','shy','sibling','sick','side','siege','sight','sign',
  'silent','silk','silly','silver','similar','simple','since','sing','siren',
  'sister','situate','six','size','skate','sketch','ski','skill','skin',
  'skirt','skull','slab','slam','sleep','slender','slice','slide','slight',
  'slim','slogan','slot','slow','slush','small','smart','smile','smoke',
  'smooth','snack','snake','snap','sniff','snow','soap','soccer','social',
  'sock','soda','soft','solar','soldier','solid','solution','solve','someone',
  'song','soon','sorry','sort','soul','sound','soup','source','south','space',
  'spare','spatial','spawn','speak','special','speed','spell','spend','sphere',
  'spice','spider','spike','spin','spirit','split','spoil','sponsor','spoon',
  'sport','spot','spray','spread','spring','spy','square','squeeze','squirrel',
  'stable','stadium','staff','stage','stairs','stamp','stand','start','state',
  'stay','steak','steel','stem','step','stereo','stick','still','sting',
  'stock','stomach','stone','stool','story','stove','strategy','street',
  'strike','strong','struggle','student','stuff','stumble','style','subject',
  'submit','subway','success','such','sudden','suffer','sugar','suggest',
  'suit','summer','sun','sunny','sunset','super','supply','supreme','sure',
  'surface','surge','surprise','surround','survey','suspect','sustain',
  'swallow','swamp','swap','swarm','swear','sweet','swift','swim','swing',
  'switch','sword','symbol','symptom','syrup','system','table','tackle','tag',
  'tail','talent','talk','tank','tape','target','task','taste','tattoo',
  'taxi','teach','team','tell','ten','tenant','tennis','tent','term','test',
  'text','thank','that','theme','then','theory','there','they','thing','this',
  'thought','three','thrive','throw','thumb','thunder','ticket','tide','tiger',
  'tilt','timber','time','tiny','tip','tired','tissue','title','toast',
  'tobacco','today','toddler','toe','together','toilet','token','tomato',
  'tomorrow','tone','tongue','tonight','tool','tooth','top','topic','topple',
  'torch','tornado','tortoise','toss','total','tourist','toward','tower',
  'town','toy','track','trade','traffic','tragic','train','transfer','trap',
  'trash','travel','tray','treat','tree','trend','trial','tribe','trick',
  'trigger','trim','trip','trophy','trouble','truck','true','truly','trumpet',
  'trust','truth','try','tube','tuition','tumble','tuna','tunnel','turkey',
  'turn','turtle','twelve','twenty','twice','twin','twist','two','type',
  'typical','ugly','umbrella','unable','unaware','uncle','uncover','under',
  'undo','unfair','unfold','unhappy','uniform','unique','unit','universe',
  'unknown','unlock','until','unusual','unveil','update','upgrade','uphold',
  'upon','upper','upset','urban','urge','usage','use','used','useful',
  'useless','usual','utility','vacant','vacuum','vague','valid','valley',
  'valve','van','vanish','vapor','various','vast','vault','vehicle','velvet',
  'vendor','venture','venue','verb','verify','version','very','vessel',
  'veteran','viable','vibrant','vicious','victory','video','view','village',
  'vintage','violin','virtual','virus','visa','visit','visual','vital',
  'vivid','vocal','voice','void','volcano','volume','vote','voyage','wage',
  'wagon','wait','walk','wall','walnut','want','warfare','warm','warrior',
  'wash','wasp','waste','water','wave','way','wealth','weapon','wear',
  'weasel','weather','web','wedding','weekend','weird','welcome','west','wet',
  'whale','what','wheat','wheel','when','where','whip','whisper','wide',
  'width','wife','wild','will','win','window','wine','wing','wink','winner',
  'winter','wire','wisdom','wise','wish','witness','wolf','woman','wonder',
  'wood','wool','word','work','world','worry','worth','wrap','wreck',
  'wrestle','wrist','write','wrong','yard','year','yellow','you','young',
  'youth','zebra','zero','zone','zoo',
];

// ─── Encoding helpers (mirror identity.js) ─────────────────────────────────

/**
 * Base64URL encode — no padding, URL-safe.
 */
function toBase64URL(buf) {
  return Buffer.from(buf).toString('base64url');
}

/**
 * Decode Base64URL to Buffer.
 */
function fromBase64URL(str) {
  return Buffer.from(str, 'base64url');
}

/**
 * Hex encode.
 */
function toHex(buf) {
  return Buffer.from(buf).toString('hex');
}

/**
 * Hex decode.
 */
function fromHex(hex) {
  return Buffer.from(hex, 'hex');
}

// ─── BIP39: Entropy ↔ Mnemonic (BigInt-based) ─────────────────────────────

/**
 * Convert entropy bytes to a BIP39 mnemonic phrase.
 *
 * @param {Buffer} entropyBytes - 16 bytes (128-bit) or 32 bytes (256-bit)
 * @returns {string} Space-separated mnemonic (12 or 24 words)
 * @throws {Error} If entropy length is invalid
 */
function entropyToMnemonic(entropyBytes) {
  const entropyLen = entropyBytes.length;
  if (entropyLen !== 16 && entropyLen !== 32) {
    throw new Error(`Invalid entropy length: ${entropyLen} bytes (expected 16 or 32)`);
  }

  const entropyBits = entropyLen * 8;
  const checksumBits = entropyBits / 32;

  const hash = crypto.createHash('sha256').update(entropyBytes).digest();

  // Build a BigInt from entropy bytes + checksum bits
  let value = 0n;
  for (const b of entropyBytes) {
    value = (value << 8n) | BigInt(b);
  }
  // Append checksum (first checksumBits of the hash)
  const checksum = hash[0] >> (8 - checksumBits);
  value = (value << BigInt(checksumBits)) | BigInt(checksum);

  // Extract 11-bit word indices
  const totalBits = entropyBits + checksumBits;
  const wordCount = totalBits / 11;
  const words = [];
  for (let i = wordCount - 1; i >= 0; i--) {
    const idx = Number(value & 0x7FFn);
    words.unshift(BIP39_WORDS[idx]);
    value = value >> 11n;
  }

  return words.join(' ');
}

/**
 * Convert a BIP39 mnemonic phrase back to entropy bytes.
 *
 * @param {string} mnemonic - Space-separated mnemonic (12 or 24 words)
 * @returns {Buffer} The entropy bytes (16 or 32 bytes)
 * @throws {Error} If any word is invalid or checksum fails
 */
function mnemonicToEntropy(mnemonic) {
  const words = mnemonic.trim().split(/\s+/);
  const wordCount = words.length;
  if (wordCount !== 12 && wordCount !== 24) {
    throw new Error(`Invalid word count: ${wordCount} (expected 12 or 24)`);
  }

  const totalBits = wordCount * 11;
  const checksumBits = totalBits / 33; // total = 33 * checksumBits
  const entropyBits = totalBits - checksumBits;
  const entropyBytes = entropyBits / 8;

  // Reconstruct BigInt from word indices
  let value = 0n;
  for (const word of words) {
    const idx = BIP39_WORDS.indexOf(word);
    if (idx === -1) {
      throw new Error(`Invalid BIP39 word: "${word}"`);
    }
    value = (value << 11n) | BigInt(idx);
  }

  // Extract checksum (last checksumBits)
  const checksumMask = (1n << BigInt(checksumBits)) - 1n;
  const checksumFromMnemonic = Number(value & checksumMask);
  value = value >> BigInt(checksumBits);

  // Extract entropy bytes
  const entropy = Buffer.alloc(entropyBytes);
  for (let i = entropyBytes - 1; i >= 0; i--) {
    entropy[i] = Number(value & 0xFFn);
    value = value >> 8n;
  }

  // Verify checksum
  const hash = crypto.createHash('sha256').update(entropy).digest();
  const expectedChecksum = hash[0] >> (8 - checksumBits);
  if (checksumFromMnemonic !== expectedChecksum) {
    throw new Error('Mnemonic checksum mismatch — phrase is invalid');
  }

  return entropy;
}

// ─── BIP39: Seed derivation ───────────────────────────────────────────────

/**
 * Derive a 64-byte seed from a mnemonic using PBKDF2 (BIP39 standard).
 *
 * @param {string} mnemonic
 * @param {string} [passphrase=''] - Optional passphrase (BIP39 passphrase)
 * @returns {Buffer} 64-byte seed
 */
function mnemonicToSeed(mnemonic, passphrase) {
  const normalizedMnemonic = mnemonic.normalize('NFKD');
  const normalizedPassphrase = (passphrase || '').normalize('NFKD');
  const salt = 'mnemonic' + normalizedPassphrase;
  return crypto.pbkdf2Sync(
    normalizedMnemonic,
    salt,
    2048,       // iterations (BIP39 standard)
    64,         // key length
    'sha512',
  );
}

// ─── Public API ────────────────────────────────────────────────────────────

/**
 * Generate a BIP39 mnemonic seed phrase.
 *
 * @param {number} [strength=128] - Entropy strength: 128 → 12 words, 256 → 24 words
 * @returns {string} Space-separated mnemonic phrase
 * @throws {Error} If strength is not 128 or 256
 */
function generateMnemonic(strength) {
  if (strength === undefined) strength = 128;
  if (strength !== 128 && strength !== 256) {
    throw new Error(`Invalid strength: ${strength} (expected 128 or 256)`);
  }
  const entropyBytes = strength / 8;
  const entropy = crypto.randomBytes(entropyBytes);
  return entropyToMnemonic(entropy);
}

/**
 * Derive an Ed25519 keypair from a BIP39 mnemonic + optional passphrase.
 *
 * Deterministic — same mnemonic + passphrase always produces the same keys.
 *
 * @param {string} mnemonic - BIP39 seed phrase
 * @param {string} [passphrase=''] - Optional BIP39 passphrase
 * @returns {{ pubkey: string, privkey: string, pubkeyHex: string }}
 *   Same shape as identity.js generateKeyPair()
 */
function mnemonicToKeypair(mnemonic, passphrase) {
  const seed = mnemonicToSeed(mnemonic, passphrase);

  // Use the first 32 bytes as the Ed25519 seed for tweetnacl
  const edSeed = seed.slice(0, 32);
  const kp = nacl.sign.keyPair.fromSeed(new Uint8Array(edSeed));

  return {
    pubkey: toBase64URL(Buffer.from(kp.publicKey)),
    privkey: toBase64URL(Buffer.from(kp.secretKey)),
    pubkeyHex: toHex(Buffer.from(kp.publicKey)),
  };
}

/**
 * Validate a BIP39 mnemonic phrase.
 *
 * Checks that all words are in the BIP39 English wordlist and the
 * embedded checksum is correct.
 *
 * @param {string} mnemonic - The phrase to validate
 * @returns {boolean} true if the mnemonic is valid
 */
function validateMnemonic(mnemonic) {
  if (!mnemonic || typeof mnemonic !== 'string') return false;
  const trimmed = mnemonic.trim();
  if (!trimmed) return false;

  try {
    mnemonicToEntropy(trimmed);
    return true;
  } catch {
    return false;
  }
}

/**
 * Validate a mnemonic and return detailed error information on failure.
 *
 * @param {string} mnemonic
 * @returns {{ valid: boolean, error?: string }}
 */
function validateMnemonicDetailed(mnemonic) {
  if (!mnemonic || typeof mnemonic !== 'string') {
    return { valid: false, error: 'Mnemonic must be a non-empty string' };
  }
  const trimmed = mnemonic.trim();
  if (!trimmed) {
    return { valid: false, error: 'Mnemonic is empty' };
  }

  const words = trimmed.split(/\s+/);
  if (words.length !== 12 && words.length !== 24) {
    return { valid: false, error: `Expected 12 or 24 words, got ${words.length}` };
  }

  for (const word of words) {
    if (BIP39_WORDS.indexOf(word) === -1) {
      return { valid: false, error: `Word not in BIP39 English wordlist: "${word}"` };
    }
  }

  try {
    mnemonicToEntropy(trimmed);
    return { valid: true };
  } catch (err) {
    return { valid: false, error: err.message };
  }
}

// ─── Encryption at rest (AES-256-GCM) ──────────────────────────────────────

const ENCRYPTION_ALGO = 'aes-256-gcm';
const IV_LENGTH = 12; // 96-bit IV (recommended for GCM)
const PBKDF2_ITERATIONS = 100000;
const ENC_SALT = 'mosaic-key-encryption-v1';

/**
 * Derive a 256-bit AES key from a passphrase.
 */
function deriveEncryptionKey(passphrase) {
  return crypto.pbkdf2Sync(passphrase, ENC_SALT, PBKDF2_ITERATIONS, 32, 'sha512');
}

/**
 * Encrypt a private key for storage at rest.
 *
 * @param {string} privkey - Base64URL-encoded 64-byte private key
 * @param {string} passphrase - Encryption passphrase
 * @returns {{ encrypted: string, iv: string, tag: string }}
 *   All fields are Base64URL-encoded.
 */
function encryptPrivkey(privkey, passphrase) {
  const key = deriveEncryptionKey(passphrase);
  const iv = crypto.randomBytes(IV_LENGTH);
  const cipher = crypto.createCipheriv(ENCRYPTION_ALGO, key, iv);

  const privkeyBytes = fromBase64URL(privkey);
  const encrypted = Buffer.concat([cipher.update(privkeyBytes), cipher.final()]);
  const tag = cipher.getAuthTag();

  return {
    encrypted: toBase64URL(encrypted),
    iv: toBase64URL(iv),
    tag: toBase64URL(tag),
  };
}

/**
 * Decrypt a private key that was encrypted with encryptPrivkey().
 *
 * @param {string} encrypted - Base64URL-encoded ciphertext
 * @param {string} iv - Base64URL-encoded 12-byte IV
 * @param {string} tag - Base64URL-encoded 16-byte GCM auth tag
 * @param {string} passphrase - Encryption passphrase
 * @returns {string} Base64URL-encoded 64-byte private key
 * @throws {Error} If passphrase is wrong or data is corrupted
 */
function decryptPrivkey(encrypted, iv, tag, passphrase) {
  const key = deriveEncryptionKey(passphrase);
  const decipher = crypto.createDecipheriv(
    ENCRYPTION_ALGO,
    key,
    fromBase64URL(iv),
  );
  decipher.setAuthTag(fromBase64URL(tag));

  const decrypted = Buffer.concat([
    decipher.update(fromBase64URL(encrypted)),
    decipher.final(),
  ]);

  return toBase64URL(decrypted);
}

// ─── Social Recovery (Shamir's Secret Sharing over GF(256)) ────────────────

/*
 * Implements Shamir's Secret Sharing over GF(2^8) with the irreducible
 * polynomial x^8 + x^4 + x^3 + x + 1 (0x11B), as used in AES.
 *
 * Each byte of the secret is independently split into N shares such that
 * any M shares (threshold) can reconstruct the original secret.
 */

// Pre-computed GF(256) log and exp tables
const GF_LOG = new Uint8Array(256);
const GF_EXP = new Uint8Array(512);

(function initGF() {
  let x = 1;
  for (let i = 0; i < 255; i++) {
    GF_EXP[i] = x;
    GF_LOG[x] = i;
    // Multiply by 3 (0x03) — the full generator of GF(256) with poly 0x11B
    // 3 * x = (x+1) * x = x^2 + x = (x<<1 ^ poly) ^ x
    const x2 = (x << 1) ^ (x >= 128 ? 0x11B : 0);
    x = x ^ x2;
    x &= 0xFF;
  }
  // Duplicate exp table for overflow-free lookup
  for (let i = 255; i < 512; i++) {
    GF_EXP[i] = GF_EXP[i - 255];
  }
})();

/** Multiply two GF(256) elements. */
function gfMul(a, b) {
  if (a === 0 || b === 0) return 0;
  return GF_EXP[GF_LOG[a] + GF_LOG[b]];
}

/** Divide a by b in GF(256). */
function gfDiv(a, b) {
  if (b === 0) throw new Error('Division by zero in GF(256)');
  if (a === 0) return 0;
  return GF_EXP[GF_LOG[a] - GF_LOG[b] + 255];
}

/** Add (XOR) two GF(256) elements. */
function gfAdd(a, b) { return a ^ b; }

/**
 * Evaluate a polynomial f(x) = c0 + c1*x + c2*x^2 + ... using Horner's method.
 */
function polyEval(coeffs, x) {
  let result = 0;
  for (let i = coeffs.length - 1; i >= 0; i--) {
    result = gfMul(result, x) ^ coeffs[i];
  }
  return result;
}

/**
 * Lagrange interpolation to recover f(0) given a set of (x, y) points.
 */
function lagrangeInterpolate(points) {
  // points: array of { x: number, y: number }
  let result = 0;
  for (let i = 0; i < points.length; i++) {
    let numerator = 1;
    let denominator = 1;
    for (let j = 0; j < points.length; j++) {
      if (i === j) continue;
      numerator = gfMul(numerator, points[j].x);
      denominator = gfMul(denominator, gfAdd(points[j].x, points[i].x));
    }
    result = gfAdd(result, gfMul(points[i].y, gfDiv(numerator, denominator)));
  }
  return result;
}

/**
 * Generate N-of-M Shamir shares from a seed.
 *
 * @param {Buffer|string} seed - The secret to split (Buffer or hex string)
 * @param {number} total - Total number of shares to create (M)
 * @param {number} threshold - Minimum shares needed to recover (N)
 * @returns {Array<{ index: number, data: string }>}
 *   Each share has an index (1-based) and hex-encoded data (same length as seed).
 */
function generateShares(seed, total, threshold) {
  if (threshold < 2) throw new Error('Threshold must be >= 2');
  if (threshold > total) throw new Error('Threshold must be <= total shares');
  if (total > 255) throw new Error('Maximum 255 shares');

  const secretBuf = typeof seed === 'string' ? fromHex(seed) : Buffer.from(seed);
  if (secretBuf.length === 0) throw new Error('Seed must not be empty');

  const shares = [];

  for (let shareIdx = 0; shareIdx < total; shareIdx++) {
    shares.push([]);
  }

  for (let byteIdx = 0; byteIdx < secretBuf.length; byteIdx++) {
    // Create random polynomial of degree (threshold - 1)
    // f(x) = secret + c1*x + c2*x^2 + ... + c{t-1}*x^{t-1}
    const coeffs = [secretBuf[byteIdx]];
    for (let j = 1; j < threshold; j++) {
      coeffs.push(crypto.randomBytes(1)[0]);
    }

    // Evaluate at x = 1 .. total
    for (let x = 1; x <= total; x++) {
      shares[x - 1].push(polyEval(coeffs, x));
    }
  }

  return shares.map((s, i) => ({
    index: i + 1,
    data: toHex(Buffer.from(s)),
  }));
}

/**
 * Recover a seed from a set of Shamir shares.
 *
 * @param {Array<{ index: number, data: string }>} shares
 *   At least `threshold` shares, each with an index and hex data of equal length.
 * @returns {Buffer} The recovered seed bytes.
 */
function recoverFromShares(shares) {
  if (!shares || shares.length < 2) {
    throw new Error('At least 2 shares are required');
  }

  const dataLen = shares[0].data.length / 2; // hex bytes
  const points = shares.map(s => ({
    x: s.index,
    data: fromHex(s.data),
  }));

  // Verify all data lengths match
  for (const p of points) {
    if (p.data.length !== dataLen) {
      throw new Error('All shares must have the same data length');
    }
  }

  const result = Buffer.alloc(dataLen);
  for (let byteIdx = 0; byteIdx < dataLen; byteIdx++) {
    const pts = points.map(p => ({ x: p.x, y: p.data[byteIdx] }));
    result[byteIdx] = lagrangeInterpolate(pts);
  }

  return result;
}

// ─── Exports ───────────────────────────────────────────────────────────────

module.exports = {
  // BIP39 mnemonic generation and validation
  generateMnemonic,
  validateMnemonic,
  validateMnemonicDetailed,

  // Key derivation
  mnemonicToKeypair,
  mnemonicToSeed,
  mnemonicToEntropy,
  entropyToMnemonic,

  // Encryption at rest
  encryptPrivkey,
  decryptPrivkey,

  // Social recovery (Shamir's Secret Sharing)
  generateShares,
  recoverFromShares,

  // Encoders (compatible with identity.js)
  toBase64URL,
  fromBase64URL,
  toHex,
  fromHex,
};
