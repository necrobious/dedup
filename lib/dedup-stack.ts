import * as cdk from 'aws-cdk-lib';
import * as iam from 'aws-cdk-lib/aws-iam';
import * as dynamodb from 'aws-cdk-lib/aws-dynamodb';
import * as lambda from 'aws-cdk-lib/aws-lambda';
import * as logs from 'aws-cdk-lib/aws-logs';
import { Construct } from 'constructs';

export class DedupStack extends cdk.Stack {
  constructor(scope: Construct, id: string, props?: cdk.StackProps) {
    super(scope, id, props);

    const fnPath = this.node.tryGetContext("dedup:lambda:path");

    const table = new dynamodb.TableV2(this, 'DedupTable', {
      tableName: 'Dedup',
      partitionKey: { name: 'pk', type: dynamodb.AttributeType.STRING },
      removalPolicy: cdk.RemovalPolicy.DESTROY,
      timeToLiveAttribute: 'exp',
    });

    const fnExecRole = new iam.Role(this,"DedupExecRole", {
      assumedBy: new iam.ServicePrincipal("lambda.amazonaws.com"),
      description: "lambda execution role for the Dedup function",
    });     
    fnExecRole.addManagedPolicy(iam.ManagedPolicy.fromAwsManagedPolicyName("service-role/AWSLambdaBasicExecutionRole"));

    table.grantReadWriteData(fnExecRole);

    const fn = new lambda.Function(this, 'DedupFn', {
      //architecture: lambda.Architecture.ARM_64,
      architecture: lambda.Architecture.ARM_64,
      runtime: lambda.Runtime.PROVIDED_AL2023,
      tracing: lambda.Tracing.ACTIVE,
      timeout: cdk.Duration.seconds(60),
      handler: "bootstrap", // name.othername pattern required, else will cause runtime cfn error with obscure error
      role: fnExecRole,
      code: lambda.Code.fromAsset(fnPath),
      logRetention: logs.RetentionDays.ONE_WEEK, // TODO look into using the new pattern in latest CDK
      environment: {
        RUST_LOG: "info",
      },
    });

    const fnUrl = fn.addFunctionUrl({
      authType: lambda.FunctionUrlAuthType.NONE,
      invokeMode: lambda.InvokeMode.BUFFERED,
    })

    new cdk.CfnOutput(this, "DedupFnUrl", {
      value: fnUrl.url,
      exportName: "DedupFnUrl",
    });


  }
}
