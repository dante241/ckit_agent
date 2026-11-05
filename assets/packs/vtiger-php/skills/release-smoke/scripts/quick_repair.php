<?php
// quick_repair.php <root> — run Vtiger_QuickRepair_Action in <root> and print its outcome, column-type sync lines included.
// cli_tool.php -c quick_repair hides both the output (ob_clean) and TypeErrors (catch Exception only).
chdir($argv[1]);
$src = file_get_contents('cli_tool.php');
eval('?>' . substr($src, 0, strpos($src, '// Parse options')));

$action = new Vtiger_QuickRepair_Action();
ob_start();
$started = microtime(true);
try {
	$action->process(new Vtiger_Request([]));
	$status = 'OK';
}
catch (Throwable $e) {
	$status = 'THROWN ' . get_class($e) . ': ' . $e->getMessage() . ' @ ' . $e->getFile() . ':' . $e->getLine();
}
$text = html_entity_decode(strip_tags(str_replace('<br/>', "\n", ob_get_clean())));

echo 'QuickRepair ' . $status . ' in ' . round(microtime(true) - $started, 1) . "s\n";
foreach (explode("\n", $text) as $line) {
	if (preg_match('/New fields|\* |failed|error|Exception|Done!/i', $line)) echo '  ' . trim($line) . "\n";
}
exit($status === 'OK' ? 0 : 1);
